//! 内嵌 jq 引擎（jaq，纯 Rust）驱动 `query --filter`。in-process 执行 jq 过滤器，
//! 不 shell-out 到外部 `jq` 二进制（避免每次调用的进程启动开销，符合速度目标）。
//!
//! API 依 jaq-core 3.x（见 crate 文档示例）：Loader → Arena → Compiler → Ctx → run。

use anyhow::{Result, anyhow};
use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Vars, data, unwrap_valr};
use jaq_json::{Val, read};

/// 对 `input_json`（一段 JSON 文本）执行 jq `filter`，返回输出值序列（转回 serde_json::Value）。
pub fn run_filter(input_json: &str, filter: &str) -> Result<Vec<serde_json::Value>> {
    let input =
        read::parse_single(input_json.as_bytes()).map_err(|e| anyhow!("failed to parse input JSON: {e:?}"))?;

    let program = File { code: filter, path: () };
    let defs = jaq_core::defs().chain(jaq_std::defs()).chain(jaq_json::defs());
    let funs = jaq_core::funs().chain(jaq_std::funs()).chain(jaq_json::funs());

    let loader = Loader::new(defs);
    let arena = Arena::default();
    let modules = loader
        .load(&arena, program)
        .map_err(|_| anyhow!("jq filter syntax error: {filter}"))?;

    let compiled = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|_| anyhow!("jq filter compilation failed: {filter}"))?;

    let ctx = Ctx::<data::JustLut<Val>>::new(&compiled.lut, Vars::new([]));
    let outputs = compiled.id.run((ctx, input)).map(unwrap_valr);

    let mut results = Vec::new();
    for item in outputs {
        let val = item.map_err(|e| anyhow!("jq execution error: {e:?}"))?;
        // Val 的 Display 即紧凑 JSON；转回 serde_json::Value 供上层反序列化为 Bookmark。
        let v: serde_json::Value =
            serde_json::from_str(&val.to_string()).map_err(|e| anyhow!("jq output cannot be parsed as JSON: {e}"))?;
        results.push(v);
    }
    Ok(results)
}
