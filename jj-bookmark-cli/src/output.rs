//! 输出渲染：`--json`（第二集成契约 §11）与人类可读列表。

use crate::model::Bookmark;
use anyhow::{Context, Result};
use serde::Serialize;

/// `--json` 输出：顶层 `{version, bookmarks}`，字段同文件格式，UTF-8 不转义，pretty。
/// 失败向上返回错误（由 main 以非零码 + stderr 报告），绝不吐半截 JSON。
pub fn print_json(bms: &[Bookmark], version: u32) -> Result<()> {
    #[derive(Serialize)]
    struct Out<'a> {
        version: u32,
        bookmarks: &'a [Bookmark],
    }
    let s = serde_json::to_string_pretty(&Out { version, bookmarks: bms })
        .context("failed to serialize --json output")?;
    println!("{s}");
    Ok(())
}

/// 人类可读列表（stdout 内容行 + stderr 计数），供终端浏览；App/脚本请用 `--json`。
pub fn print_human(bms: &[Bookmark]) {
    for b in bms {
        let folder = if b.folder.is_empty() { "Uncategorized" } else { &b.folder };
        println!("{}  {}", b.id, b.title);
        println!("      {}  ·  {}  ·  {}", folder, b.created_jst, b.url);
    }
    let n = bms.len();
    eprintln!("({n} {})", if n == 1 { "item" } else { "items" });
}
