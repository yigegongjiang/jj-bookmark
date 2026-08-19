//! jj-bookmark CLI — 唯一核心。读写协议 / 查询只在此实现一遍，
//! App 经内嵌调用复用。命令见 roadmap.md，数据契约见 data-model.md。

mod model;
mod output;
mod query;
mod store;
mod sync;
mod timeutil;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};

use model::{Bookmark, CURRENT_VERSION, DEFAULT_SOURCE, FOLDER_SEP, Store, now_millis};
use query::{Order, SortKey, is_ancestor};
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
    /// Target source [default: default]
    #[arg(long, value_name = "NAME", value_parser = parse_source, global = true)]
    source: Option<String>,
}

impl ScopeArgs {
    fn is_explicit(&self) -> bool {
        self.source.is_some()
    }

    /// 每条命令恒作用于单一 source；无 `--source` = `default`（= `apply <URL>` 的落点）。
    fn resolve(self) -> String {
        self.source.unwrap_or_else(|| DEFAULT_SOURCE.to_owned())
    }
}

fn not_found(source: &str, id: i64) -> anyhow::Error {
    anyhow!("bookmark #{id} not found in source {source:?}")
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
        /// Delete the bookmark identified by TARGET (tombstone; restore with --restore)
        #[arg(long)]
        delete: bool,
        /// Bring a deleted bookmark back
        #[arg(long)]
        restore: bool,
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
        /// List deleted bookmarks (tombstones) instead of live ones
        #[arg(long)]
        deleted: bool,
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
    /// Sync with the cloud copy: pull, merge, push (the web can write too)
    Sync,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::resolve()?;
    let explicit_scope = cli.scope.is_explicit();
    let source = cli.scope.resolve();
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
            restore,
        } => {
            if delete && restore {
                bail!("--delete and --restore are mutually exclusive");
            }
            if let Ok(id) = target.parse::<i64>() {
                if delete || restore {
                    if title.is_some()
                        || url.is_some()
                        || folder.is_some()
                        || note.is_some()
                        || excerpt.is_some()
                        || set_source.is_some()
                    {
                        let flag = if delete { "--delete" } else { "--restore" };
                        bail!("{flag} cannot be combined with fields");
                    }
                    cmd_set_deleted(&paths, id, &source, delete)
                } else {
                    cmd_edit(
                        &paths, id, title, url, folder, note, excerpt, set_source, &source,
                    )
                }
            } else {
                if delete || restore {
                    bail!("--delete/--restore require a numeric bookmark ID");
                }
                if url.is_some() || set_source.is_some() {
                    bail!("--url/--set-source require a numeric bookmark ID");
                }
                cmd_add(&paths, target, title, folder, note, excerpt, &source)
            }
        }
        Command::Ls {
            keyword,
            folder,
            sort,
            order,
            deleted,
            json,
        } => cmd_list(&paths, keyword, folder, sort, order, deleted, json, &source),
        Command::Folders => cmd_folders(&paths, &source),
        Command::Sources => {
            if explicit_scope {
                bail!("--source cannot be used with sources; it always lists every source");
            }
            cmd_sources(&paths)
        }
        Command::Open { id } => cmd_open(&paths, id, &source),
        Command::Mv { old, new } => cmd_mv(&paths, old, new, &source),
        Command::Sync => {
            if explicit_scope {
                bail!("--source cannot be used with sync; sync always covers every source");
            }
            sync::sync(&paths)
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
    source: &str,
) -> Result<()> {
    let title = title.unwrap_or_else(|| url.clone()); // 无 --title 时用 URL 占位
    let folder = folder.unwrap_or_default();
    let note = note.unwrap_or_default();
    let id = mutate(paths, |store| {
        if let Some(existing) = store.sources.get(source) {
            ensure_leaf_placement(&folder, live_folders(existing, None))?;
        }
        let id = unique_id(store);
        let mut bookmark =
            Bookmark::new(id, url.clone(), title.clone(), folder.clone(), note.clone());
        if let Some(excerpt) = &excerpt {
            bookmark.excerpt = excerpt.clone();
        }
        store
            .sources
            .entry(source.to_owned())
            .or_default()
            .push(bookmark);
        Ok(id)
    })?;
    println!("Added #{id}");
    Ok(())
}

/// 读命令的 `--source X` 指向不存在的 source 时报错并列出已知 source（静默空结果会让 typo
/// 看起来「库里没有」）。`default` 恒合法（它是默认落点，空库亦然）。
fn ensure_known_source(store: &Store, source: &str) -> Result<()> {
    if source == DEFAULT_SOURCE || store.sources.contains_key(source) {
        return Ok(());
    }
    let known: Vec<&str> = store.sources.keys().map(String::as_str).collect();
    bail!("unknown source {source:?} (known: {})", known.join(", "))
}

fn cmd_folders(paths: &Paths, source: &str) -> Result<()> {
    let store = read_store(paths)?;
    ensure_known_source(&store, source)?;
    let mut folders: Vec<_> = store
        .sources
        .get(source)
        .map(|bookmarks| {
            bookmarks
                .iter()
                .filter(|b| !b.deleted)
                .map(|b| b.folder.clone())
                .collect()
        })
        .unwrap_or_else(Vec::new);
    folders.retain(|folder| !folder.is_empty());
    folders.sort();
    folders.dedup();
    for folder in folders {
        println!("{folder}");
    }
    Ok(())
}

fn cmd_sources(paths: &Paths) -> Result<()> {
    for (source, bookmarks) in read_store(paths)?.sources {
        // 墓碑不计入主数字（它们对用户不可见），仅在存在时附注，便于察觉库在膨胀。
        let live = bookmarks.iter().filter(|b| !b.deleted).count();
        match bookmarks.len() - live {
            0 => println!("{source}\t{live}"),
            tombstones => println!("{source}\t{live}\t(+{tombstones} deleted)"),
        }
    }
    Ok(())
}

fn cmd_list(
    paths: &Paths,
    keyword: Option<String>,
    folder: Option<String>,
    sort: SortKey,
    order: Option<Order>,
    deleted: bool,
    json: bool,
    source: &str,
) -> Result<()> {
    let mut store = read_store(paths)?;
    ensure_known_source(&store, source)?;
    let mut bookmarks = store.sources.remove(source).unwrap_or_default();
    // 墓碑对所有读取方不可见（App / raycast 都经此 `--json` 契约消费，故只在这里过滤一次）；
    // `--deleted` 反过来只看墓碑，用于确认误删与 `apply <ID> --restore`。
    bookmarks.retain(|b| b.deleted == deleted);
    if let Some(keyword) = &keyword {
        bookmarks = query::keyword_filter(bookmarks, keyword);
    }
    if let Some(folder) = &folder {
        bookmarks = query::folder_filter(bookmarks, folder);
    }
    let order = order.unwrap_or_else(|| sort.default_order());
    query::sort_bookmarks(&mut bookmarks, sort, order);
    if json {
        // `--json` 契约恒为 {version, sources:{<source>:[...]}}（此处单键），供 jq / 脚本消费。
        output::print_json(source, &bookmarks, CURRENT_VERSION)?;
    } else {
        output::print_human(&bookmarks);
    }
    Ok(())
}

fn cmd_open(paths: &Paths, id: i64, source: &str) -> Result<()> {
    let url = mutate(paths, |store| {
        let b = store
            .sources
            .get_mut(source)
            .and_then(|bookmarks| {
                bookmarks
                    .iter_mut()
                    .find(|bookmark| bookmark.id == id && !bookmark.deleted)
            })
            .ok_or_else(|| not_found(source, id))?;
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
    source: &str,
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
        let origin = source.to_owned();
        let target = set_source.clone().unwrap_or_else(|| origin.clone());

        // 叶子约束：仅当放置变化（换 source 或换 folder）时，校验最终落点；
        // 纯字段编辑（folder 不变）不触发，避免追溯惩罚既有放置。
        let current_folder = store
            .sources
            .get(&origin)
            .ok_or_else(|| not_found(&origin, id))?
            .iter()
            .find(|b| b.id == id && !b.deleted)
            .map(|b| b.folder.clone())
            .ok_or_else(|| not_found(&origin, id))?;
        let new_folder = folder.clone().unwrap_or_else(|| current_folder.clone());
        if target != origin || new_folder != current_folder {
            let existing: Vec<&str> = store
                .sources
                .get(&target)
                .map(|v| live_folders(v, Some(id)).collect())
                .unwrap_or_default();
            ensure_leaf_placement(&new_folder, existing.into_iter())?;
        }

        if target == origin {
            let bookmark = store
                .sources
                .get_mut(&origin)
                .and_then(|bookmarks| {
                    bookmarks
                        .iter_mut()
                        .find(|bookmark| bookmark.id == id && !bookmark.deleted)
                })
                .ok_or_else(|| not_found(&origin, id))?;
            apply_edit_fields(bookmark, &title, &url, &folder, &note, &excerpt);
        } else {
            let bookmarks = store.sources.get_mut(&origin).expect("source exists");
            let index = bookmarks
                .iter()
                .position(|bookmark| bookmark.id == id && !bookmark.deleted)
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

/// 软删除 / 恢复（data-model §12）。删除不物理移除记录，只置 `deleted` 并刷新 `updated`——
/// 墓碑是 sync 合并的正确性前提：没有它，「本地删了、远端没删」会在下次合并时复活该条。
///
/// 恢复不做叶子校验：它只是把记录还原到删除前的状态，那个状态当初就是合法的
/// （与「既有放置不追溯」同一策略），并且校验失败会让误删无法撤销。
fn cmd_set_deleted(paths: &Paths, id: i64, source: &str, deleted: bool) -> Result<()> {
    mutate(paths, |store| {
        let bookmark = store
            .sources
            .get_mut(source)
            .and_then(|bookmarks| {
                bookmarks
                    .iter_mut()
                    .find(|bookmark| bookmark.id == id && bookmark.deleted != deleted)
            })
            .ok_or_else(|| {
                if deleted {
                    not_found(source, id)
                } else {
                    anyhow!("bookmark #{id} is not deleted in source {source:?}")
                }
            })?;
        bookmark.deleted = deleted;
        bookmark.updated = now_millis(); // LWW 依据：删除 / 恢复都是内容变更，必须抬时间戳
        Ok(())
    })?;
    println!("{} #{id}", if deleted { "Deleted" } else { "Restored" });
    Ok(())
}

fn cmd_mv(paths: &Paths, old: String, new: String, source: &str) -> Result<()> {
    let n = mutate(paths, |store| {
        let prefix = format!("{old}{FOLDER_SEP}");
        let now = now_millis();
        let no_match = || anyhow!("no folder matches {old:?} in source {source:?}");
        let bookmarks = store.sources.get_mut(source).ok_or_else(no_match)?;
        let mut relocated: Vec<String> = Vec::new();
        for bookmark in bookmarks.iter_mut().filter(|b| !b.deleted) {
            let new_folder = if bookmark.folder == old {
                new.clone()
            } else if let Some(rest) = bookmark.folder.strip_prefix(&prefix) {
                format!("{new}{FOLDER_SEP}{rest}")
            } else {
                continue;
            };
            bookmark.folder = new_folder.clone();
            bookmark.updated = now;
            relocated.push(new_folder);
        }
        if relocated.is_empty() {
            return Err(no_match());
        }
        // 叶子约束：每个被移动到的 folder 不得与本 source 内任一 folder 互为祖先。
        // 只校验涉及被移动落点的冲突，既有的无关脏数据不触发。
        let all: Vec<&str> = live_folders(bookmarks, None).collect();
        for folder in &relocated {
            ensure_leaf_placement(folder, all.iter().copied())?;
        }
        Ok(relocated.len())
    })?;
    println!("Moved {n} bookmark(s): {old} → {new}");
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

/// 同 source 内**未删除**条目的 folder 路径（可排除某个 id，用于「编辑自身不与自身冲突」）。
/// 墓碑不占 folder，故一律不参与叶子约束判定。
fn live_folders(bookmarks: &[Bookmark], skip_id: Option<i64>) -> impl Iterator<Item = &str> {
    bookmarks
        .iter()
        .filter(move |b| !b.deleted && Some(b.id) != skip_id)
        .map(|b| b.folder.as_str())
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
    fn scope_defaults_to_default_source_and_accepts_both_positions() {
        let cli = Cli::try_parse_from(["jj-bookmark", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), DEFAULT_SOURCE);

        let cli = Cli::try_parse_from(["jj-bookmark", "--source", "safari", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), "safari");

        // 子命令之后写同样成立（global arg），两种位置等价。
        let cli = Cli::try_parse_from(["jj-bookmark", "ls", "--source", "safari"]).unwrap();
        assert_eq!(cli.scope.resolve(), "safari");
        let cli = Cli::try_parse_from(["jj-bookmark", "folders", "--source", "safari"]).unwrap();
        assert_eq!(cli.scope.resolve(), "safari");
        assert!(Cli::try_parse_from(["jj-bookmark", "--all", "ls"]).is_err());

        let command = Cli::command();
        assert!(command.get_arguments().any(|arg| arg.get_id() == "source"));
        assert!(command.get_arguments().all(|arg| arg.get_id() != "all"));
    }

    #[test]
    fn unknown_source_errors_instead_of_empty_output() {
        let paths = temp_paths("unknown-source");
        seed(&paths, DEFAULT_SOURCE, 1, "A");
        let error = list(&paths, false, "nosuch").unwrap_err();
        assert!(error.to_string().contains("unknown source"));
        assert!(cmd_folders(&paths, "nosuch").is_err());
        assert!(cmd_folders(&paths, DEFAULT_SOURCE).is_ok());
        let _ = fs::remove_dir_all(&paths.dir);
    }

    /// 无 `--source` 时新增落 `default`；`--source <NEW>` 即创建该 source。
    #[test]
    fn add_lands_in_the_named_source() {
        let paths = temp_paths("add-default");
        let add = |source: &str| {
            cmd_add(
                &paths,
                "https://example.com".into(),
                None,
                None,
                None,
                None,
                source,
            )
        };
        add(DEFAULT_SOURCE).unwrap();
        add("safari").unwrap();
        let store = read_store(&paths).unwrap();
        assert_eq!(
            store.sources.keys().collect::<Vec<_>>(),
            [DEFAULT_SOURCE, "safari"]
        );
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn source_is_trimmed_and_empty_is_rejected() {
        let cli = Cli::try_parse_from(["jj-bookmark", "--source", " safari ", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), "safari");

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

        let default_scope = DEFAULT_SOURCE;
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
            "safari",
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

    fn default_scope() -> &'static str {
        DEFAULT_SOURCE
    }

    fn list(paths: &Paths, deleted: bool, source: &str) -> Result<()> {
        cmd_list(paths, None, None, SortKey::Created, None, deleted, false, source)
    }

    /// 某 source 内可见（未删）与墓碑的 id。
    fn ids(paths: &Paths, source: &str) -> (Vec<i64>, Vec<i64>) {
        let store = read_store(paths).unwrap();
        let bookmarks = store.sources.get(source).cloned().unwrap_or_default();
        (
            bookmarks.iter().filter(|b| !b.deleted).map(|b| b.id).collect(),
            bookmarks.iter().filter(|b| b.deleted).map(|b| b.id).collect(),
        )
    }

    #[test]
    fn delete_leaves_a_tombstone_that_restore_can_bring_back() {
        let paths = temp_paths("soft-delete");
        seed(&paths, DEFAULT_SOURCE, 1, "A::B");
        seed(&paths, DEFAULT_SOURCE, 2, "A::C");

        cmd_set_deleted(&paths, 1, DEFAULT_SOURCE, true).unwrap();
        assert_eq!(ids(&paths, DEFAULT_SOURCE), (vec![2], vec![1]), "记录仍在，只是被标记");

        // 已删条目对读 / 写命令一律不可见
        assert!(cmd_edit(&paths, 1, Some("x".into()), None, None, None, None, None, DEFAULT_SOURCE).is_err());
        assert!(cmd_open(&paths, 1, DEFAULT_SOURCE).is_err());
        assert!(cmd_set_deleted(&paths, 1, DEFAULT_SOURCE, true).is_err(), "重复删除应报错");

        // 墓碑仍占 id：新增绝不会复用它（否则会与远端墓碑撞 id）
        let store = read_store(&paths).unwrap();
        assert!(store.contains_id(1));

        cmd_set_deleted(&paths, 1, DEFAULT_SOURCE, false).unwrap();
        assert_eq!(ids(&paths, DEFAULT_SOURCE), (vec![1, 2], vec![]));
        assert!(cmd_set_deleted(&paths, 1, DEFAULT_SOURCE, false).is_err(), "未删条目不能恢复");
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn tombstones_do_not_occupy_folders_or_show_up_in_listings() {
        let paths = temp_paths("tombstone-folders");
        seed(&paths, DEFAULT_SOURCE, 1, "A::B::C");
        cmd_set_deleted(&paths, 1, DEFAULT_SOURCE, true).unwrap();

        // A::B::C 已成墓碑 → 祖先 A::B 重新可挂
        assert!(
            cmd_add(&paths, "u".into(), None, Some("A::B".into()), None, None, DEFAULT_SOURCE).is_ok()
        );
        // folders 不再列出墓碑的路径
        cmd_folders(&paths, DEFAULT_SOURCE).unwrap();
        let store = read_store(&paths).unwrap();
        let live: Vec<&str> = store.sources[DEFAULT_SOURCE]
            .iter()
            .filter(|b| !b.deleted)
            .map(|b| b.folder.as_str())
            .collect();
        assert_eq!(live, vec!["A::B"]);
        // ls 默认只看活的，--deleted 只看墓碑
        assert!(list(&paths, false, DEFAULT_SOURCE).is_ok());
        assert!(list(&paths, true, DEFAULT_SOURCE).is_ok());
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn mv_skips_tombstones() {
        let paths = temp_paths("mv-tombstone");
        seed(&paths, DEFAULT_SOURCE, 1, "Old");
        seed(&paths, DEFAULT_SOURCE, 2, "Old");
        cmd_set_deleted(&paths, 2, DEFAULT_SOURCE, true).unwrap();

        cmd_mv(&paths, "Old".into(), "New".into(), DEFAULT_SOURCE).unwrap();
        let store = read_store(&paths).unwrap();
        let by_id = |id: i64| {
            store.sources[DEFAULT_SOURCE]
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .folder
                .clone()
        };
        assert_eq!(by_id(1), "New");
        assert_eq!(by_id(2), "Old", "墓碑不参与 mv");
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn sync_subcommand_replaced_push() {
        assert!(Cli::command().find_subcommand("push").is_none());
        assert!(Cli::command().find_subcommand("sync").is_some());
        assert!(Cli::try_parse_from(["jj-bookmark", "apply", "1", "--restore"]).is_ok());
        // --delete 与 --restore 互斥（在 main 分发处 bail，此处只确认二者都可解析）
        assert!(Cli::try_parse_from(["jj-bookmark", "apply", "1", "--delete", "--restore"]).is_ok());
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
                "safari",
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
