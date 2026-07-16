//! raindrop CSV 导入（roadmap Phase 1）。按表头名取列，容错处理引号/逗号/换行
//! （由 `csv` crate 保证）。created 为 UTC ISO8601 → epoch 毫秒 + JST 派生；
//! updated 同 created；last_visited=0。保留 raindrop 原 id。

use crate::model::{Bookmark, now_millis};
use crate::timeutil::parse_iso8601_to_millis;
use anyhow::{Context, Result};
use std::path::Path;

/// 解析 raindrop CSV → Bookmark 列表（不落库，落库由调用方在写锁内 upsert）。
pub fn parse_raindrop_csv(path: &Path) -> Result<Vec<Bookmark>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("打开 CSV 失败: {}", path.display()))?;

    let headers = rdr.headers().context("读取 CSV 表头失败")?.clone();
    let col = |name: &str| headers.iter().position(|h| h == name);
    let (ci_id, ci_title, ci_note, ci_excerpt, ci_url, ci_folder, ci_tags, ci_created, ci_cover, ci_fav) = (
        col("id"), col("title"), col("note"), col("excerpt"), col("url"),
        col("folder"), col("tags"), col("created"), col("cover"), col("favorite"),
    );

    let mut out = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.with_context(|| format!("解析 CSV 第 {} 行失败", i + 2))?;
        let get = |ci: Option<usize>| ci.and_then(|c| rec.get(c)).unwrap_or("").to_string();

        let url = get(ci_url);
        if url.is_empty() {
            continue; // 无 URL 视为坏行，跳过
        }

        let id = get(ci_id).trim().parse::<i64>().unwrap_or_else(|_| now_millis());
        let created = parse_iso8601_to_millis(&get(ci_created)).unwrap_or_else(|_| now_millis());
        let tags = get(ci_tags)
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let favorite = matches!(get(ci_fav).trim(), "true" | "1" | "yes");

        let mut b = Bookmark {
            id,
            title: get(ci_title),
            url,
            excerpt: get(ci_excerpt),
            note: get(ci_note),
            folder: get(ci_folder),
            cover: get(ci_cover),
            tags,
            favorite,
            created,
            created_jst: String::new(),
            updated: created,
            updated_jst: String::new(),
            last_visited: 0,
            last_visited_jst: String::new(),
        };
        b.normalize_jst();
        out.push(b);
    }
    Ok(out)
}
