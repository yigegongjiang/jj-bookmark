//! Push：把本地 `bookmarks.json` 单向上传到 Cloudflare R2（经 wrangler CLI）。
//!
//! 只 push、无 pull —— web 侧只读，数据源恒为 CLI/本地文件。上传的是**磁盘原文件**
//! （已由每次 `mutate` 归一为 pretty JSON），不在此重新序列化；上传前仅做一次解析校验，
//! 避免把损坏文件推上去。

use crate::store::{Paths, read_store};
use anyhow::{Context, Result, bail};
use std::process::Command;

/// R2 目标固定常量（bucket + object key），与 web Worker（`src/index.js` 的 `R2_KEY`）
/// 逐字对齐。单向同步、单一数据源，无需可变，写死最简、最不易漂移。
pub const BUCKET: &str = "jj-bookmark";
pub const KEY: &str = "bookmarks.json";

/// 解析 wrangler 调用：程序名 + 前置参数。
///
/// PATH 上有全局 `wrangler` 就用它，否则兜底 `npx wrangler`（本机常见形态：无全局、经 npx）。
/// 零配置，无需 env。
pub fn resolve_wrangler() -> (String, Vec<String>) {
    if wrangler_on_path() {
        return ("wrangler".into(), Vec::new());
    }
    ("npx".into(), vec!["wrangler".into()])
}

/// PATH 各目录里是否存在可执行的 `wrangler`（不 shell-out 到 `which`）。
fn wrangler_on_path() -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| {
        let cand = dir.join("wrangler");
        cand.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    })
}

/// 拼装 `wrangler r2 object put` 参数（纯函数，便于测试）。
pub fn build_put_args(base: &[String], bucket: &str, key: &str, file: &str) -> Vec<String> {
    let mut args: Vec<String> = base.to_vec();
    args.extend([
        "r2".into(),
        "object".into(),
        "put".into(),
        format!("{bucket}/{key}"),
        "--file".into(),
        file.into(),
        "--content-type".into(),
        "application/json".into(),
        "--remote".into(), // 命中真实 R2，而非 wrangler 本地模拟
    ]);
    args
}

/// 校验并上传本地数据文件到固定 R2 目标（`BUCKET`/`KEY`）。wrangler 的 stdout/stderr 直接透传。
pub fn push(paths: &Paths) -> Result<()> {
    if !paths.data.exists() {
        bail!("no data file to push yet: {} (add a bookmark first)", paths.data.display());
    }
    read_store(paths)?; // 上传前解析校验：损坏文件不推上去（保留原错误信息）
    let file = paths.data.to_str().context("data file path is not valid UTF-8")?;

    let (prog, base) = resolve_wrangler();
    let args = build_put_args(&base, BUCKET, KEY, file);

    let status = Command::new(&prog)
        .args(&args)
        .status()
        .with_context(|| format!("failed to run `{prog}` (is wrangler / node installed and on PATH?)"))?;
    if !status.success() {
        bail!(
            "wrangler upload failed (exit {}). Check `wrangler login` and that R2 bucket `{BUCKET}` exists.",
            status.code().unwrap_or(-1),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_args_shape() {
        let args = build_put_args(&[], "jj-bookmark", "bookmarks.json", "/tmp/b.json");
        assert_eq!(
            args,
            vec![
                "r2",
                "object",
                "put",
                "jj-bookmark/bookmarks.json",
                "--file",
                "/tmp/b.json",
                "--content-type",
                "application/json",
                "--remote",
            ]
        );
    }

    #[test]
    fn put_args_prepends_base() {
        // base（如 npx 的 "wrangler"）须排在子命令之前
        let args = build_put_args(&["wrangler".into()], "b", "k", "f");
        assert_eq!(&args[..4], &["wrangler", "r2", "object", "put"]);
        assert_eq!(args[4], "b/k");
    }

    #[test]
    fn target_constants_align_with_worker() {
        // bucket/key 为固定常量，须与 web Worker 的 R2_KEY / bucket_name 一致
        assert_eq!(BUCKET, "jj-bookmark");
        assert_eq!(KEY, "bookmarks.json");
    }
}
