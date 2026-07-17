//! grouped JSON 与人类可读输出。

use crate::model::Bookmark;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;

pub fn print_json(sources: &BTreeMap<String, Vec<Bookmark>>, version: u32) -> Result<()> {
    #[derive(Serialize)]
    struct Out<'a> {
        version: u32,
        sources: &'a BTreeMap<String, Vec<Bookmark>>,
    }
    let json = serde_json::to_string_pretty(&Out { version, sources })
        .context("failed to serialize --json output")?;
    println!("{json}");
    Ok(())
}

pub fn print_human(sources: &BTreeMap<String, Vec<Bookmark>>, show_groups: bool) {
    for (source, bookmarks) in sources {
        if show_groups && !bookmarks.is_empty() {
            println!("[{source}]");
        }
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
    }
    let count: usize = sources.values().map(Vec::len).sum();
    eprintln!("({count} {})", if count == 1 { "item" } else { "items" });
}
