//! 排序与关键词过滤（data-model §10）。四个固定排序键与关键词模糊搜均为原生比较，
//! 不经 jq 引擎（jaq 仅服务 `--filter`，见 filter.rs）。

use crate::model::Bookmark;
use clap::ValueEnum;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum SortKey {
    Created,
    Updated,
    Visited,
    Title,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Order {
    Asc,
    Desc,
}

impl SortKey {
    /// 各键的默认方向：title 升序，其余降序。
    pub fn default_order(self) -> Order {
        match self {
            SortKey::Title => Order::Asc,
            _ => Order::Desc,
        }
    }
}

/// 原地排序。次级键固定 `id` 降序，保证顺序确定（不依赖排序稳定性或原始次序）。
pub fn sort_bookmarks(bms: &mut [Bookmark], key: SortKey, order: Order) {
    bms.sort_by(|a, b| {
        let primary = match key {
            SortKey::Created => a.created.cmp(&b.created),
            SortKey::Updated => a.updated.cmp(&b.updated),
            SortKey::Visited => a.last_visited.cmp(&b.last_visited),
            SortKey::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        };
        let primary = match order {
            Order::Asc => primary,
            Order::Desc => primary.reverse(),
        };
        primary.then_with(|| b.id.cmp(&a.id)) // 次级键：id 降序
    });
}

/// 多关键词搜索：按 Unicode 空白分词；每词大小写不敏感；全部词均须命中任意可搜索字段。
pub fn keyword_filter(bms: Vec<Bookmark>, keyword: &str) -> Vec<Bookmark> {
    let terms: Vec<String> = keyword.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return bms;
    }
    bms.into_iter()
        .filter(|b| {
            let searchable = format!(
                "{} {} {} {} {} {} {}",
                b.source,
                b.title,
                b.url,
                b.excerpt,
                b.note,
                b.folder,
                b.tags.join(" ")
            )
            .to_lowercase();
            terms.iter().all(|term| searchable.contains(term))
        })
        .collect()
}

/// folder 子树过滤：命中该 folder 本身或其后代（前缀 + " / "）。
pub fn folder_filter(bms: Vec<Bookmark>, folder: &str) -> Vec<Bookmark> {
    let prefix = format!("{folder} / ");
    bms.into_iter()
        .filter(|b| b.folder == folder || b.folder.starts_with(&prefix))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bm(id: i64, created: i64, visited: i64, title: &str) -> Bookmark {
        let mut b = Bookmark::new(id, "u".into(), title.into(), "".into(), "".into());
        b.created = created;
        b.last_visited = visited;
        b
    }

    #[test]
    fn sort_created_desc_default() {
        let mut v = vec![bm(1, 100, 0, "a"), bm(2, 300, 0, "b"), bm(3, 200, 0, "c")];
        sort_bookmarks(&mut v, SortKey::Created, Order::Desc);
        assert_eq!(v.iter().map(|b| b.id).collect::<Vec<_>>(), vec![2, 3, 1]);
    }

    #[test]
    fn sort_tiebreak_id_desc() {
        // created 全相等 → 次级键 id 降序，结果确定
        let mut v = vec![bm(1, 100, 0, "a"), bm(3, 100, 0, "b"), bm(2, 100, 0, "c")];
        sort_bookmarks(&mut v, SortKey::Created, Order::Desc);
        assert_eq!(v.iter().map(|b| b.id).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn sort_visited_zero_sinks() {
        let mut v = vec![bm(1, 0, 0, "a"), bm(2, 0, 500, "b"), bm(3, 0, 0, "c")];
        sort_bookmarks(&mut v, SortKey::Visited, Order::Desc);
        assert_eq!(v[0].id, 2); // 最近访问在最前，未访问(0)沉底
    }

    #[test]
    fn keyword_matches_url_and_excerpt() {
        let mut a = bm(1, 0, 0, "hello");
        a.url = "https://Markdown.com".into();
        let b = bm(2, 0, 0, "world");
        let out = keyword_filter(vec![a, b], "markdown");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, 1);
    }

    #[test]
    fn keyword_matches_all_whitespace_separated_terms() {
        let mut matching = bm(1, 0, 0, "Dashboard");
        matching.url = "https://example.com".into();
        let missing_term = bm(2, 0, 0, "Dashboard");

        let out = keyword_filter(vec![matching, missing_term], "  dashboard\u{3000}COM  ");

        assert_eq!(out.iter().map(|b| b.id).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn keyword_matches_note_folder_and_tags() {
        let mut matching = bm(1, 0, 0, "Reference");
        matching.note = "Rust ownership".into();
        matching.folder = "Work / Backend".into();
        matching.tags = vec!["language".into()];

        let out = keyword_filter(vec![matching], "rust backend language");

        assert_eq!(out.len(), 1);
    }

    #[test]
    fn folder_subtree() {
        let mut a = bm(1, 0, 0, "a");
        a.folder = "Work".into();
        let mut b = bm(2, 0, 0, "b");
        b.folder = "Work / Docs".into();
        let mut c = bm(3, 0, 0, "c");
        c.folder = "Tools".into();
        let out = folder_filter(vec![a, b, c], "Work");
        assert_eq!(out.len(), 2);
    }
}
