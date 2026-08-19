//! grouped JSON 与人类可读输出。

use crate::model::Bookmark;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;

/// 契约形状恒为 `{version, sources:{<source>:[...]}}`（单 source 单键），与数据文件同构。
pub fn print_json(source: &str, bookmarks: &[Bookmark], version: u32) -> Result<()> {
    #[derive(Serialize)]
    struct Out<'a> {
        version: u32,
        sources: BTreeMap<&'a str, &'a [Bookmark]>,
    }
    let sources = BTreeMap::from([(source, bookmarks)]);
    let json = serde_json::to_string_pretty(&Out { version, sources })
        .context("failed to serialize --json output")?;
    println!("{json}");
    Ok(())
}

pub fn print_human(bookmarks: &[Bookmark]) {
    for bookmark in bookmarks {
        let folder = if bookmark.folder.is_empty() {
            "Uncategorized"
        } else {
            &bookmark.folder
        };
        println!("{}  {}", bookmark.id, bookmark.title);
        println!(
            "      {}  ·  {}  ·  {}",
            folder, bookmark.created_jst, bookmark.url
        );
    }
    let count = bookmarks.len();
    eprintln!("({count} {})", if count == 1 { "item" } else { "items" });
}
