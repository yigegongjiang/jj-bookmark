//! 数据模型：磁盘与 `--json` 均使用 `{ version, sources: { name: [...] } }`。

use crate::timeutil::epoch_millis_to_jst;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CURRENT_VERSION: u32 = 3;
pub const DEFAULT_SOURCE: &str = "default";
/// folder 层级分隔符：无空格，避免 AI 写路径时被空格挤压致格式漂移。段名不得含此串。
pub const FOLDER_SEP: &str = "::";

fn default_source() -> String {
    DEFAULT_SOURCE.to_owned()
}

fn normalize_source(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        default_source()
    } else {
        value.to_owned()
    }
}

/// 顶层结构。BTreeMap 令 source 顺序与 JSON 输出稳定。
#[derive(Debug, Clone, Serialize)]
pub struct Store {
    pub version: u32,
    pub sources: BTreeMap<String, Vec<Bookmark>>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            version: CURRENT_VERSION,
            sources: BTreeMap::new(),
        }
    }
}

/// 兼容读取 v1/v2 的扁平 `{bookmarks:[{source,...}]}`，下一次写入自动升级为 v3。
impl<'de> Deserialize<'de> for Store {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireStore {
            #[serde(default)]
            version: u32,
            #[serde(default)]
            sources: Option<BTreeMap<String, Vec<Bookmark>>>,
            #[serde(default)]
            bookmarks: Option<Vec<LegacyBookmark>>,
        }

        #[derive(Deserialize)]
        struct LegacyBookmark {
            #[serde(default = "default_source")]
            source: String,
            #[serde(flatten)]
            bookmark: Bookmark,
        }

        let wire = WireStore::deserialize(deserializer)?;
        if wire.sources.is_some() && wire.bookmarks.is_some() {
            return Err(D::Error::custom(
                "data file contains both sources and bookmarks",
            ));
        }

        let mut sources = BTreeMap::<String, Vec<Bookmark>>::new();
        if let Some(grouped) = wire.sources {
            for (source, mut bookmarks) in grouped {
                sources
                    .entry(normalize_source(&source))
                    .or_default()
                    .append(&mut bookmarks);
            }
        } else if let Some(bookmarks) = wire.bookmarks {
            for legacy in bookmarks {
                sources
                    .entry(normalize_source(&legacy.source))
                    .or_default()
                    .push(legacy.bookmark);
            }
        }
        sources.retain(|_, bookmarks| !bookmarks.is_empty());
        Ok(Store {
            version: wire.version,
            sources,
        })
    }
}

impl Store {
    /// 写入前升级 schema、归一 source key、清理空组、重算派生时间。
    pub fn normalize(&mut self) {
        self.version = CURRENT_VERSION;
        let mut normalized = BTreeMap::<String, Vec<Bookmark>>::new();
        for (source, mut bookmarks) in std::mem::take(&mut self.sources) {
            for bookmark in &mut bookmarks {
                bookmark.normalize_jst();
            }
            normalized
                .entry(normalize_source(&source))
                .or_default()
                .append(&mut bookmarks);
        }
        normalized.retain(|_, bookmarks| !bookmarks.is_empty());
        self.sources = normalized;
    }

    #[cfg(test)]
    pub fn total_len(&self) -> usize {
        self.sources.values().map(Vec::len).sum()
    }

    pub fn contains_id(&self, id: i64) -> bool {
        self.sources
            .values()
            .any(|bookmarks| bookmarks.iter().any(|bookmark| bookmark.id == id))
    }
}

/// 单条书签。source 只存在于顶层 map key，不在每条记录内重复。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    #[serde(default)]
    pub id: i64,
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
    pub fn new(id: i64, url: String, title: String, folder: String, note: String) -> Self {
        let now = now_millis();
        let mut bookmark = Bookmark {
            id,
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
        bookmark.normalize_jst();
        bookmark
    }

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

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_flat_store_is_grouped_without_serialized_source_fields() {
        let raw = serde_json::json!({
            "version": 2,
            "bookmarks": [
                { "id": 1, "source": "safari", "title": "s" },
                { "id": 2, "title": "d" }
            ]
        });
        let mut store: Store = serde_json::from_value(raw).unwrap();
        assert_eq!(store.sources["default"][0].id, 2);
        assert_eq!(store.sources["safari"][0].id, 1);

        store.normalize();
        let grouped = serde_json::to_value(store).unwrap();
        assert_eq!(grouped["version"], CURRENT_VERSION);
        assert!(grouped.get("bookmarks").is_none());
        assert!(grouped["sources"]["safari"][0].get("source").is_none());
    }

    #[test]
    fn normalize_merges_blank_source_into_default_and_removes_empty_groups() {
        let mut store = Store::default();
        store.sources.insert(
            " ".into(),
            vec![Bookmark::new(
                1,
                "u".into(),
                "t".into(),
                "".into(),
                "".into(),
            )],
        );
        store.sources.insert("empty".into(), Vec::new());
        store.normalize();
        assert_eq!(store.total_len(), 1);
        assert_eq!(
            store.sources.keys().cloned().collect::<Vec<_>>(),
            vec![DEFAULT_SOURCE]
        );
    }
}
