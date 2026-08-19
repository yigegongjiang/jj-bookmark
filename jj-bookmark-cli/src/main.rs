//! jj-bookmark CLI — 唯一核心。读写协议 / 查询只在此实现一遍，
//! App 经内嵌调用复用。命令见 roadmap.md，数据契约见 data-model.md。

mod model;
mod output;
mod pusher;
mod query;
mod store;
mod timeutil;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use std::collections::BTreeMap;

use model::{Bookmark, CURRENT_VERSION, DEFAULT_SOURCE, FOLDER_SEP, Store, now_millis};
use query::{Order, SortKey};
use store::{Paths, mutate, read_store};

#[derive(Parser)]
#[command(
    name = "jj-bookmark",
    version,
    about = "Bookmark tool",
    disable_help_subcommand = true,
    before_help = "TL;DR — save a bookmark:\n  1. Search: jj-bookmark ls <DOMAIN>; same domain = strong match; ask: add or edit <ID>?\n  2. Pick the closest path from jj-bookmark folders; levels are joined by `::` with no spaces (e.g. AI::claude-code); infer title and useful metadata.\n  3. jj-bookmark apply <URL> --title <TITLE> --folder <PATH> [--note <NOTE>] [--excerpt <TEXT>]\n     <URL> = the exact current page URL, verbatim (full path + query); NEVER substitute the bare domain or a trimmed form.\n  Edit: jj-bookmark apply <ID> <fields>; delete: jj-bookmark apply <ID> --delete."
)]
struct Cli {
    #[command(flatten)]
    scope: ScopeArgs,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Args, Clone, Debug)]
struct ScopeArgs {
    /// Narrow to one source (default: all sources; `apply <URL>` without it adds to `default`)
    #[arg(long, value_name = "NAME", value_parser = parse_source)]
    source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Scope {
    Source(String),
    All,
}

impl ScopeArgs {
    fn is_explicit(&self) -> bool {
        self.source.is_some()
    }

    /// 无 `--source` = 全部 source（读与按 ID 写的常态；id 全局唯一，见 `unique_id`）。
    fn resolve(self) -> Scope {
        self.source.map_or(Scope::All, Scope::Source)
    }
}

impl Scope {
    fn includes_source(&self, candidate: &str) -> bool {
        match self {
            Scope::Source(source) => source == candidate,
            Scope::All => true,
        }
    }

    fn not_found(&self, id: i64) -> anyhow::Error {
        match self {
            Scope::Source(source) => anyhow!("bookmark #{id} not found in source {source:?}"),
            Scope::All => anyhow!("bookmark #{id} not found"),
        }
    }
}

fn parse_source(value: &str) -> std::result::Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err("source must not be empty".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

#[derive(Subcommand)]
enum Command {
    /// Add, edit, or delete a bookmark
    Apply {
        /// URL to add, or bookmark ID to edit/delete
        target: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        url: Option<String>,
        /// Folder path; nest levels with `::` and no spaces (e.g. AI::claude-code)
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        excerpt: Option<String>,
        /// Move an existing bookmark to another source
        #[arg(long, value_name = "NAME", value_parser = parse_source)]
        set_source: Option<String>,
        /// Delete the bookmark identified by TARGET
        #[arg(long)]
        delete: bool,
    },
    /// List bookmarks, or search by whitespace-separated keywords (sortable)
    Ls {
        /// Keyword(s) to match title/url/excerpt/note/folder; omit to list all
        keyword: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long, value_enum, default_value_t = SortKey::Created)]
        sort: SortKey,
        #[arg(long, value_enum)]
        order: Option<Order>,
        /// Emit the --json contract (consumed by the App/scripts)
        #[arg(long)]
        json: bool,
    },
    /// List existing folder paths, one per line
    Folders,
    /// List all sources and bookmark counts
    Sources,
    /// Open the URL in the default browser and record the last visit
    Open { id: i64 },
    /// Rename / move a folder subtree (prefix-replace all matches, single atomic write)
    Mv {
        /// Old folder path (including all its descendants)
        old: String,
        /// New folder path
        new: String,
    },
    /// Push the local data file to Cloudflare R2 (one-way; the web is read-only)
    Push,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::resolve()?;
    let explicit_scope = cli.scope.is_explicit();
    let scope = cli.scope.resolve();
    match cli.cmd {
        Command::Apply {
            target,
            title,
            url,
            folder,
            note,
            excerpt,
            set_source,
            delete,
        } => {
            if let Ok(id) = target.parse::<i64>() {
                if delete {
                    if title.is_some()
                        || url.is_some()
                        || folder.is_some()
                        || note.is_some()
                        || excerpt.is_some()
                        || set_source.is_some()
                    {
                        bail!("--delete cannot be combined with fields");
                    }
                    cmd_rm(&paths, id, scope)
                } else {
                    cmd_edit(
                        &paths, id, title, url, folder, note, excerpt, set_source, scope,
                    )
                }
            } else {
                if delete {
                    bail!("--delete requires a numeric bookmark ID");
                }
                if url.is_some() || set_source.is_some() {
                    bail!("--url/--set-source require a numeric bookmark ID");
                }
                cmd_add(&paths, target, title, folder, note, excerpt, scope)
            }
        }
        Command::Ls {
            keyword,
            folder,
            sort,
            order,
            json,
        } => cmd_list(&paths, keyword, folder, sort, order, json, scope),
        Command::Folders => cmd_folders(&paths, scope),
        Command::Sources => {
            if explicit_scope {
                bail!("--source cannot be used with sources; it always lists every source");
            }
            cmd_sources(&paths)
        }
        Command::Open { id } => cmd_open(&paths, id, scope),
        Command::Mv { old, new } => cmd_mv(&paths, old, new, scope),
        Command::Push => {
            if explicit_scope {
                bail!("--source cannot be used with push; push always uploads all sources");
            }
            cmd_push(&paths)
        }
    }
}

fn cmd_add(
    paths: &Paths,
    url: String,
    title: Option<String>,
    folder: Option<String>,
    note: Option<String>,
    excerpt: Option<String>,
    scope: Scope,
) -> Result<()> {
    // 新增须有唯一落点：无 `--source` 落 DEFAULT_SOURCE（读侧的「全部」在此收敛为默认组）。
    let source = match scope {
        Scope::Source(source) => source,
        Scope::All => DEFAULT_SOURCE.to_owned(),
    };
    let title = title.unwrap_or_else(|| url.clone()); // 无 --title 时用 URL 占位
    let folder = folder.unwrap_or_default();
    let note = note.unwrap_or_default();
    let id = mutate(paths, |store| {
        if let Some(existing) = store.sources.get(&source) {
            ensure_leaf_placement(&folder, existing.iter().map(|b| b.folder.as_str()))?;
        }
        let id = unique_id(store);
        let mut bookmark =
            Bookmark::new(id, url.clone(), title.clone(), folder.clone(), note.clone());
        if let Some(excerpt) = &excerpt {
            bookmark.excerpt = excerpt.clone();
        }
        store
            .sources
            .entry(source.clone())
            .or_default()
            .push(bookmark);
        Ok(id)
    })?;
    println!("Added #{id}");
    Ok(())
}

fn cmd_folders(paths: &Paths, scope: Scope) -> Result<()> {
    let store = read_store(paths)?;
    let mut folders: Vec<_> = store
        .sources
        .into_iter()
        .filter(|(source, _)| scope.includes_source(source))
        .flat_map(|(_, bookmarks)| bookmarks.into_iter().map(|bookmark| bookmark.folder))
        .filter(|folder| !folder.is_empty())
        .collect();
    folders.sort();
    folders.dedup();
    for folder in folders {
        println!("{folder}");
    }
    Ok(())
}

fn cmd_sources(paths: &Paths) -> Result<()> {
    for (source, bookmarks) in read_store(paths)?.sources {
        println!("{source}\t{}", bookmarks.len());
    }
    Ok(())
}

fn cmd_list(
    paths: &Paths,
    keyword: Option<String>,
    folder: Option<String>,
    sort: SortKey,
    order: Option<Order>,
    json: bool,
    scope: Scope,
) -> Result<()> {
    let store = read_store(paths)?;
    let mut sources: BTreeMap<_, _> = store
        .sources
        .into_iter()
        .filter(|(source, _)| scope.includes_source(source))
        .collect();
    let order = order.unwrap_or_else(|| sort.default_order());
    for bookmarks in sources.values_mut() {
        let mut filtered = std::mem::take(bookmarks);
        if let Some(keyword) = &keyword {
            filtered = query::keyword_filter(filtered, keyword);
        }
        if let Some(folder) = &folder {
            filtered = query::folder_filter(filtered, folder);
        }
        query::sort_bookmarks(&mut filtered, sort, order);
        *bookmarks = filtered;
    }
    sources.retain(|_, bookmarks| !bookmarks.is_empty());
    if json {
        output::print_json(&sources, CURRENT_VERSION)?;
    } else {
        output::print_human(&sources, matches!(scope, Scope::All));
    }
    Ok(())
}

fn cmd_open(paths: &Paths, id: i64, scope: Scope) -> Result<()> {
    let url = mutate(paths, |store| {
        let source = source_for_id(store, id, &scope).ok_or_else(|| scope.not_found(id))?;
        let b = store
            .sources
            .get_mut(&source)
            .and_then(|bookmarks| bookmarks.iter_mut().find(|bookmark| bookmark.id == id))
            .ok_or_else(|| scope.not_found(id))?;
        b.last_visited = now_millis(); // 记录访问；不改 updated（访问 ≠ 内容修改）
        Ok(b.url.clone())
    })?;
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .with_context(|| format!("failed to invoke system open: {url}"))?;
    println!("Opened #{id}: {url}");
    Ok(())
}

fn cmd_edit(
    paths: &Paths,
    id: i64,
    title: Option<String>,
    url: Option<String>,
    folder: Option<String>,
    note: Option<String>,
    excerpt: Option<String>,
    set_source: Option<String>,
    scope: Scope,
) -> Result<()> {
    if title.is_none()
        && url.is_none()
        && folder.is_none()
        && note.is_none()
        && excerpt.is_none()
        && set_source.is_none()
    {
        bail!(
            "no fields provided to modify (--title/--url/--folder/--note/--excerpt/--set-source)"
        );
    }
    mutate(paths, |store| {
        let origin = source_for_id(store, id, &scope).ok_or_else(|| scope.not_found(id))?;
        let target = set_source.clone().unwrap_or_else(|| origin.clone());

        // 叶子约束：仅当放置变化（换 source 或换 folder）时，校验最终落点；
        // 纯字段编辑（folder 不变）不触发，避免追溯惩罚既有放置。
        let current_folder = store.sources[&origin]
            .iter()
            .find(|b| b.id == id)
            .map(|b| b.folder.clone())
            .ok_or_else(|| scope.not_found(id))?;
        let new_folder = folder.clone().unwrap_or_else(|| current_folder.clone());
        if target != origin || new_folder != current_folder {
            let existing: Vec<&str> = store
                .sources
                .get(&target)
                .map(|v| {
                    v.iter()
                        .filter(|b| b.id != id)
                        .map(|b| b.folder.as_str())
                        .collect()
                })
                .unwrap_or_default();
            ensure_leaf_placement(&new_folder, existing.into_iter())?;
        }

        if target == origin {
            let bookmark = store
                .sources
                .get_mut(&origin)
                .and_then(|bookmarks| bookmarks.iter_mut().find(|bookmark| bookmark.id == id))
                .ok_or_else(|| scope.not_found(id))?;
            apply_edit_fields(bookmark, &title, &url, &folder, &note, &excerpt);
        } else {
            let bookmarks = store.sources.get_mut(&origin).expect("source exists");
            let index = bookmarks
                .iter()
                .position(|bookmark| bookmark.id == id)
                .expect("bookmark exists");
            let mut bookmark = bookmarks.remove(index);
            apply_edit_fields(&mut bookmark, &title, &url, &folder, &note, &excerpt);
            store.sources.entry(target).or_default().push(bookmark);
        }
        Ok(())
    })?;
    println!("Updated #{id}");
    Ok(())
}

fn cmd_rm(paths: &Paths, id: i64, scope: Scope) -> Result<()> {
    mutate(paths, |store| {
        let source = source_for_id(store, id, &scope).ok_or_else(|| scope.not_found(id))?;
        store
            .sources
            .get_mut(&source)
            .expect("source exists")
            .retain(|bookmark| bookmark.id != id);
        Ok(())
    })?;
    println!("Deleted #{id}");
    Ok(())
}

fn cmd_mv(paths: &Paths, old: String, new: String, scope: Scope) -> Result<()> {
    let (n, touched) = mutate(paths, |store| {
        let prefix = format!("{old}{FOLDER_SEP}");
        let now = now_millis();
        let mut moved = 0;
        let mut touched: Vec<String> = Vec::new();
        for (source, bookmarks) in &mut store.sources {
            if !scope.includes_source(source) {
                continue;
            }
            let mut relocated: Vec<String> = Vec::new();
            for bookmark in bookmarks.iter_mut() {
                let new_folder = if bookmark.folder == old {
                    new.clone()
                } else if let Some(rest) = bookmark.folder.strip_prefix(&prefix) {
                    format!("{new}{FOLDER_SEP}{rest}")
                } else {
                    continue;
                };
                bookmark.folder = new_folder.clone();
                bookmark.updated = now;
                moved += 1;
                relocated.push(new_folder);
            }
            // 叶子约束：每个被移动到的 folder 不得与本 source 内任一 folder 互为祖先。
            // 只校验涉及被移动落点的冲突，既有的无关脏数据不触发。
            if !relocated.is_empty() {
                let all: Vec<&str> = bookmarks.iter().map(|b| b.folder.as_str()).collect();
                for folder in &relocated {
                    ensure_leaf_placement(folder, all.iter().copied())?;
                }
                touched.push(format!("{source}({})", relocated.len()));
            }
        }
        if moved == 0 {
            bail!("no folder matches {old:?}");
        }
        Ok((moved, touched))
    })?;
    println!("Moved {n} bookmark(s): {old} → {new} [{}]", touched.join(", "));
    Ok(())
}

/// 单向同步：把本地数据文件上传到固定 Cloudflare R2 目标（经 wrangler）。web 侧只读，无 pull。
fn cmd_push(paths: &Paths) -> Result<()> {
    println!(
        "Pushing {} → R2 {}/{}",
        paths.data.display(),
        pusher::BUCKET,
        pusher::KEY
    );
    pusher::push(paths)?;
    println!("Pushed to R2: {}/{}", pusher::BUCKET, pusher::KEY);
    Ok(())
}

/// 生成唯一 id：Unix 毫秒；极端同毫秒冲突则 +1 重试（data-model §7）。
fn unique_id(store: &Store) -> i64 {
    let mut id = now_millis();
    while store.contains_id(id) {
        id += 1;
    }
    id
}

fn source_for_id(store: &Store, id: i64, scope: &Scope) -> Option<String> {
    store.sources.iter().find_map(|(source, bookmarks)| {
        (scope.includes_source(source) && bookmarks.iter().any(|bookmark| bookmark.id == id))
            .then(|| source.clone())
    })
}

/// `ancestor` 是否为 `descendant` 的严格前缀祖先（按 `FOLDER_SEP` 分段）。空路径不作祖先（未分类豁免）。
fn is_ancestor(ancestor: &str, descendant: &str) -> bool {
    !ancestor.is_empty() && descendant.starts_with(&format!("{ancestor}{FOLDER_SEP}"))
}

/// 叶子挂载约束：书签只能挂到叶子 folder。校验把书签挂到 `folder` 是否与同 source 内
/// 任一已占用 folder 互为祖先（互为祖先 = 目标或其某祖先将不再是叶子）。
/// `existing` = 同 source 内其他书签的 folder 路径。空 `folder`（未分类）恒允许。
fn ensure_leaf_placement<'a>(folder: &str, existing: impl Iterator<Item = &'a str>) -> Result<()> {
    if folder.is_empty() {
        return Ok(()); // 未分类不受叶子约束（策略 A）。注：删本行不改行为——空串亦被 is_ancestor
        // 短路而恒放行；改为策略 B（必须有 folder）须在此新增对空 folder 的显式 bail!。
    }
    for other in existing {
        if other.is_empty() || other == folder {
            continue;
        }
        if is_ancestor(folder, other) {
            bail!(
                "cannot place a bookmark in non-leaf folder {folder:?}: sub-folder {other:?} exists under it; use a leaf folder"
            );
        }
        if is_ancestor(other, folder) {
            bail!(
                "cannot place a bookmark in {folder:?}: ancestor folder {other:?} already holds bookmarks and must stay a leaf"
            );
        }
    }
    Ok(())
}

fn apply_edit_fields(
    bookmark: &mut Bookmark,
    title: &Option<String>,
    url: &Option<String>,
    folder: &Option<String>,
    note: &Option<String>,
    excerpt: &Option<String>,
) {
    if let Some(title) = title {
        bookmark.title = title.clone();
    }
    if let Some(url) = url {
        bookmark.url = url.clone();
    }
    if let Some(folder) = folder {
        bookmark.folder = folder.clone();
    }
    if let Some(note) = note {
        bookmark.note = note.clone();
    }
    if let Some(excerpt) = excerpt {
        bookmark.excerpt = excerpt.clone();
    }
    bookmark.updated = now_millis();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::fs;

    fn temp_paths(tag: &str) -> Paths {
        let dir = std::env::temp_dir().join(format!(
            "jj-bookmark-main-test-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        Paths::from_dir(dir)
    }

    #[test]
    fn scope_is_root_only_and_defaults_to_all() {
        let cli = Cli::try_parse_from(["jj-bookmark", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), Scope::All);

        let cli = Cli::try_parse_from(["jj-bookmark", "--source", "safari", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), Scope::Source("safari".to_owned()));
        assert!(Cli::try_parse_from(["jj-bookmark", "ls", "--source", "safari"]).is_err());
        assert!(Cli::try_parse_from(["jj-bookmark", "--all", "ls"]).is_err());

        let command = Cli::command();
        assert!(command.get_arguments().any(|arg| arg.get_id() == "source"));
        assert!(command.get_arguments().all(|arg| arg.get_id() != "all"));
        let ls = command.find_subcommand("ls").unwrap();
        assert!(ls.get_arguments().all(|arg| arg.get_id() != "source"));
    }

    /// 读侧默认「全部 source」，但新增须有唯一落点 → 无 `--source` 落 default。
    #[test]
    fn add_without_source_lands_in_default() {
        let paths = temp_paths("add-default");
        cmd_add(
            &paths,
            "https://example.com".into(),
            None,
            None,
            None,
            None,
            Scope::All,
        )
        .unwrap();
        let store = read_store(&paths).unwrap();
        assert_eq!(store.sources.keys().collect::<Vec<_>>(), [DEFAULT_SOURCE]);
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn source_is_trimmed_and_empty_is_rejected() {
        let cli = Cli::try_parse_from(["jj-bookmark", "--source", " safari ", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), Scope::Source("safari".to_owned()));

        assert!(Cli::try_parse_from(["jj-bookmark", "--source", " ", "ls"]).is_err());
    }

    #[test]
    fn help_subcommand_is_disabled() {
        assert!(Cli::command().find_subcommand("help").is_none());
    }

    #[test]
    fn ls_takes_optional_keyword_and_query_is_gone() {
        let cli = Cli::try_parse_from(["jj-bookmark", "ls"]).unwrap();
        assert!(matches!(cli.cmd, Command::Ls { keyword: None, .. }));
        let cli = Cli::try_parse_from(["jj-bookmark", "ls", "rust lang"]).unwrap();
        assert!(matches!(cli.cmd, Command::Ls { keyword: Some(k), .. } if k == "rust lang"));
        assert!(Cli::command().find_subcommand("query").is_none());
    }

    #[test]
    fn apply_parses_add_edit_and_delete_forms() {
        for args in [
            vec!["jj-bookmark", "apply", "https://example.com"],
            vec!["jj-bookmark", "apply", "123", "--title", "new"],
            vec!["jj-bookmark", "apply", "123", "--delete"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(matches!(cli.cmd, Command::Apply { .. }));
        }
    }

    #[test]
    fn edit_finds_id_across_sources_when_scope_is_all() {
        let paths = temp_paths("edit-all");
        seed(&paths, "safari", 7, "");
        cmd_edit(
            &paths,
            7,
            Some("via-all".into()),
            None,
            None,
            None,
            None,
            None,
            Scope::All,
        )
        .unwrap();
        assert_eq!(read_store(&paths).unwrap().sources["safari"][0].title, "via-all");
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn edit_respects_scope_and_can_move_source() {
        let paths = temp_paths("edit-scope");
        mutate(&paths, |store| {
            store
                .sources
                .entry("safari".into())
                .or_default()
                .push(Bookmark::new(
                    2,
                    "u".into(),
                    "old".into(),
                    "".into(),
                    "".into(),
                ));
            Ok(())
        })
        .unwrap();

        let default_scope = Scope::Source(DEFAULT_SOURCE.to_owned());
        let error = cmd_edit(
            &paths,
            2,
            Some("blocked".into()),
            None,
            None,
            None,
            None,
            None,
            default_scope,
        )
        .unwrap_err();
        assert!(error.to_string().contains("source \"default\""));

        cmd_edit(
            &paths,
            2,
            Some("moved".into()),
            None,
            None,
            None,
            None,
            Some(DEFAULT_SOURCE.into()),
            Scope::Source("safari".into()),
        )
        .unwrap();
        let store = read_store(&paths).unwrap();
        let bookmark = &store.sources[DEFAULT_SOURCE][0];
        assert_eq!(bookmark.title, "moved");
        assert!(!store.sources.contains_key("safari"));
        let _ = fs::remove_dir_all(&paths.dir);
    }

    // 直接塞书签（绕过叶子校验），用于构造既有 / 脏数据前置状态。
    fn seed(paths: &Paths, source: &str, id: i64, folder: &str) {
        mutate(paths, |store| {
            store
                .sources
                .entry(source.into())
                .or_default()
                .push(Bookmark::new(id, "u".into(), "t".into(), folder.into(), "".into()));
            Ok(())
        })
        .unwrap();
    }

    fn default_scope() -> Scope {
        Scope::Source(DEFAULT_SOURCE.to_owned())
    }

    #[test]
    fn leaf_add_rejects_ancestor_and_descendant_allows_sibling_dup_and_uncategorized() {
        let paths = temp_paths("leaf-add");
        seed(&paths, DEFAULT_SOURCE, 1, "A::B::C"); // 现有叶子

        let add = |folder: Option<&str>| {
            cmd_add(
                &paths,
                "u".into(),
                None,
                folder.map(str::to_owned),
                None,
                None,
                default_scope(),
            )
        };
        // 挂到祖先 A::B → 拒绝
        assert!(add(Some("A::B")).is_err());
        // 挂到后代 A::B::C::D（会使 A::B::C 非叶）→ 拒绝
        assert!(add(Some("A::B::C::D")).is_err());
        // 兄弟叶子 A::B::E → 允许
        assert!(add(Some("A::B::E")).is_ok());
        // 同叶子再加一条 → 允许
        assert!(add(Some("A::B::C")).is_ok());
        // 未分类（空 folder）即使已有 folder 也允许（空 folder 豁免，策略 A）
        assert!(add(None).is_ok());
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn leaf_edit_rejects_non_leaf_but_allows_field_edit_on_dirty_folder() {
        let paths = temp_paths("leaf-edit");
        seed(&paths, DEFAULT_SOURCE, 1, "X::Y");
        seed(&paths, DEFAULT_SOURCE, 2, "Z");

        let set_folder = |id: i64, folder: &str| {
            cmd_edit(
                &paths,
                id,
                None,
                None,
                Some(folder.to_owned()),
                None,
                None,
                None,
                default_scope(),
            )
        };
        // #2 → X（X 是 X::Y 的祖先，会非叶）→ 拒绝
        assert!(set_folder(2, "X").is_err());
        // #2 → 兄弟叶子 X::K → 允许
        assert!(set_folder(2, "X::K").is_ok());

        // 脏放置：P 与 P::Q 同时占用（绕过校验直接塞）
        seed(&paths, DEFAULT_SOURCE, 3, "P");
        seed(&paths, DEFAULT_SOURCE, 4, "P::Q");
        // 仅改 #3 title（folder 不变）→ 允许（不追溯既有脏数据）
        assert!(
            cmd_edit(
                &paths,
                3,
                Some("t2".into()),
                None,
                None,
                None,
                None,
                None,
                default_scope(),
            )
            .is_ok()
        );
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn leaf_cross_source_move_into_conflict_is_rejected() {
        let paths = temp_paths("leaf-xsrc");
        seed(&paths, "safari", 1, "A"); // 待移动，folder=A
        seed(&paths, DEFAULT_SOURCE, 2, "A::B"); // 目标 source 已有 A::B
        // #1 safari→default（folder 保持 A）→ default 里 A 成 A::B 祖先 → 拒绝
        assert!(
            cmd_edit(
                &paths,
                1,
                None,
                None,
                None,
                None,
                None,
                Some(DEFAULT_SOURCE.into()),
                Scope::Source("safari".into()),
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn leaf_mv_merge_into_leaf_ok_but_into_ancestor_rejected() {
        let paths = temp_paths("leaf-mv-ok");
        seed(&paths, DEFAULT_SOURCE, 1, "A::B");
        seed(&paths, DEFAULT_SOURCE, 2, "A::C");
        // mv A::B → A::C：合并到同一叶子 → 允许
        assert!(cmd_mv(&paths, "A::B".into(), "A::C".into(), default_scope()).is_ok());
        let _ = fs::remove_dir_all(&paths.dir);

        let paths = temp_paths("leaf-mv-bad");
        seed(&paths, DEFAULT_SOURCE, 1, "A::B");
        seed(&paths, DEFAULT_SOURCE, 2, "C");
        // mv A::B → C::D：C 已占用且会成 C::D 祖先 → 拒绝
        assert!(cmd_mv(&paths, "A::B".into(), "C::D".into(), default_scope()).is_err());
        let _ = fs::remove_dir_all(&paths.dir);
    }
}
