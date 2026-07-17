//! 数据模型：与 data-model.md §2/§3 逐字对齐的 serde 类型。
//!
//! 这是「文件格式 + CLI `--json` 输出」两个集成面的唯一契约来源。字段顺序刻意
//! 与 §4 示例一致以保证输出可读；所有 `*_jst` 均为派生值，每次写入前由
//! [`Store::normalize`] 从数字主字段重新生成。

use crate::timeutil::epoch_millis_to_jst;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 schema 版本（data-model §8）。
pub const CURRENT_VERSION: u32 = 2;
pub const DEFAULT_SOURCE: &str = "default";

fn default_source() -> String {
    DEFAULT_SOURCE.to_owned()
}

/// 顶层结构 `{ version, bookmarks }`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub version: u32,
    pub bookmarks: Vec<Bookmark>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            version: CURRENT_VERSION,
            bookmarks: Vec::new(),
        }
    }
}

impl Store {
    /// 写入前归一：重算所有派生 `*_jst`，保证与数字主字段一致（含手改场景）。
    pub fn normalize(&mut self) {
        self.version = CURRENT_VERSION;
        for b in &mut self.bookmarks {
            if b.source.trim().is_empty() {
                b.source = default_source();
            }
            b.normalize_jst();
        }
    }
}

/// 单条书签。字段顺序 == data-model §4 示例。
///
/// 读取时对缺失字段一律容错兜底（`#[serde(default)]`），不因个别字段缺失拒绝整条记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    #[serde(default)]
    pub id: i64,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub excerpt: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub created_jst: String,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub updated_jst: String,
    #[serde(default)]
    pub last_visited: i64,
    #[serde(default)]
    pub last_visited_jst: String,
}

impl Bookmark {
    /// 新建书签：`created == updated == now`，`last_visited == 0`（data-model §3）。
    #[cfg(test)]
    pub fn new(id: i64, url: String, title: String, folder: String, note: String) -> Self {
        Self::new_with_source(id, default_source(), url, title, folder, note)
    }

    pub fn new_with_source(
        id: i64,
        source: String,
        url: String,
        title: String,
        folder: String,
        note: String,
    ) -> Self {
        let now = now_millis();
        let mut b = Bookmark {
            id,
            source,
            title,
            url,
            excerpt: String::new(),
            note,
            folder,
            cover: String::new(),
            tags: Vec::new(),
            favorite: false,
            created: now,
            created_jst: String::new(),
            updated: now,
            updated_jst: String::new(),
            last_visited: 0,
            last_visited_jst: String::new(),
        };
        b.normalize_jst();
        b
    }

    /// 由数字主字段重算派生 `*_jst`；`last_visited == 0`（从未访问）→ `""`。
    pub fn normalize_jst(&mut self) {
        self.created_jst = epoch_millis_to_jst(self.created);
        self.updated_jst = epoch_millis_to_jst(self.updated);
        self.last_visited_jst = if self.last_visited == 0 {
            String::new()
        } else {
            epoch_millis_to_jst(self.last_visited)
        };
    }
}

/// 当前 Unix 毫秒时间戳。
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_source_defaults_to_default() {
        let bookmark: Bookmark = serde_json::from_value(serde_json::json!({ "id": 1 })).unwrap();
        assert_eq!(bookmark.source, DEFAULT_SOURCE);
    }

    #[test]
    fn normalize_upgrades_schema_and_repairs_empty_source() {
        let mut bookmark = Bookmark::new(1, "u".into(), "t".into(), "".into(), "".into());
        bookmark.source.clear();
        let mut store = Store {
            version: 1,
            bookmarks: vec![bookmark],
        };
        store.normalize();
        assert_eq!(store.version, CURRENT_VERSION);
        assert_eq!(store.bookmarks[0].source, DEFAULT_SOURCE);
    }
}
