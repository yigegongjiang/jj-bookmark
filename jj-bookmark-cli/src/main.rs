//! jj-bookmark CLI — 唯一核心。读写协议 / 查询 / 抓取只在此实现一遍，
//! App 经内嵌调用复用。命令见 roadmap.md，数据契约见 data-model.md。

mod fetcher;
mod filter;
mod model;
mod output;
mod pusher;
mod query;
mod store;
mod timeutil;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use std::collections::BTreeMap;
use std::time::Duration;

use model::{Bookmark, CURRENT_VERSION, DEFAULT_SOURCE, Store, now_millis};
use query::{Order, SortKey};
use store::{Paths, mutate, read_store};

#[derive(Parser)]
#[command(
    name = "jj-bookmark",
    version,
    about = "Bookmark tool",
    disable_help_subcommand = true,
    before_help = "TL;DR — save a bookmark:\n  1. Search: jj-bookmark --all query <DOMAIN>; same domain = strong match; ask: add or edit <ID>?\n  2. Pick the closest path from jj-bookmark folders; infer title and useful metadata.\n  3. jj-bookmark apply <URL> --title <TITLE> --folder <PATH> [--note <NOTE>] [--excerpt <TEXT>] [--fetch]\n  Edit: jj-bookmark --all apply <ID> <fields>; delete: jj-bookmark --all apply <ID> --delete."
)]
struct Cli {
    #[command(flatten)]
    scope: ScopeArgs,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Args, Clone, Debug)]
struct ScopeArgs {
    /// Target source (default: default)
    #[arg(long, value_name = "NAME", value_parser = parse_source, conflicts_with = "all")]
    source: Option<String>,
    /// Target all sources
    #[arg(long)]
    all: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Scope {
    Source(String),
    All,
}

impl ScopeArgs {
    fn is_explicit(&self) -> bool {
        self.source.is_some() || self.all
    }

    fn resolve(self) -> Scope {
        if self.all {
            Scope::All
        } else {
            Scope::Source(self.source.unwrap_or_else(|| DEFAULT_SOURCE.to_owned()))
        }
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
        /// Fetch metadata after adding
        #[arg(long)]
        fetch: bool,
    },
    /// List bookmarks (sortable)
    Ls {
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
    /// Search bookmarks by whitespace-separated keywords (sortable); `--filter` takes a native jq filter (run by embedded jaq)
    Query {
        /// Keyword (may be an empty string to match all, used with --filter)
        keyword: String,
        /// Native jq filter; input is {version, bookmarks}, output must be bookmark objects (e.g. `.bookmarks[] | select(.favorite)`)
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long, value_enum, default_value_t = SortKey::Created)]
        sort: SortKey,
        #[arg(long, value_enum)]
        order: Option<Order>,
        #[arg(long)]
        json: bool,
    },
    /// Open the URL in the default browser and record the last visit
    Open { id: i64 },
    /// Fetch and backfill metadata (title/excerpt/cover)
    Fetch {
        id: i64,
        /// Overwrite existing fields (by default only empty fields are filled, never user content)
        #[arg(long)]
        force: bool,
    },
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
            fetch,
        } => {
            if let Ok(id) = target.parse::<i64>() {
                if delete {
                    if title.is_some()
                        || url.is_some()
                        || folder.is_some()
                        || note.is_some()
                        || excerpt.is_some()
                        || set_source.is_some()
                        || fetch
                    {
                        bail!("--delete cannot be combined with fields or --fetch");
                    }
                    cmd_rm(&paths, id, scope)
                } else {
                    if fetch {
                        bail!("--fetch is only valid when TARGET is a URL");
                    }
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
                cmd_add(&paths, target, title, folder, note, excerpt, fetch, scope)
            }
        }
        Command::Ls {
            folder,
            sort,
            order,
            json,
        } => cmd_list(&paths, None, None, folder, sort, order, json, scope),
        Command::Folders => cmd_folders(&paths, scope),
        Command::Sources => {
            if explicit_scope {
                bail!("--source/--all cannot be used with sources");
            }
            cmd_sources(&paths)
        }
        Command::Query {
            keyword,
            filter,
            folder,
            sort,
            order,
            json,
        } => cmd_list(
            &paths,
            Some(keyword),
            filter,
            folder,
            sort,
            order,
            json,
            scope,
        ),
        Command::Open { id } => cmd_open(&paths, id, scope),
        Command::Fetch { id, force } => cmd_fetch(&paths, id, force, scope),
        Command::Mv { old, new } => cmd_mv(&paths, old, new, scope),
        Command::Push => {
            if explicit_scope {
                bail!("--source/--all cannot be used with push; push always uploads all sources");
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
    fetch: bool,
    scope: Scope,
) -> Result<()> {
    let Scope::Source(source) = scope else {
        bail!("--all cannot be used when adding a bookmark");
    };
    let title = title.unwrap_or_else(|| url.clone()); // 无 --title 时用 URL 占位；抓取会回填
    let folder = folder.unwrap_or_default();
    let note = note.unwrap_or_default();
    let fetch_scope = Scope::Source(source.clone());
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
    if fetch {
        // 同步抓取；失败不回滚（书签已保存），仅告警。
        if let Err(e) = fetch_and_apply(paths, id, false, &fetch_scope) {
            eprintln!("Warning: metadata fetch failed (bookmark saved): {e:#}");
        }
    }
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

fn cmd_fetch(paths: &Paths, id: i64, force: bool, scope: Scope) -> Result<()> {
    fetch_and_apply(paths, id, force, &scope)?;
    println!("Fetched #{id}");
    Ok(())
}

/// 抓取并回填元数据。网络请求在写锁**之外**进行（避免长时间持锁阻塞其他写），
/// 抓取完成后再加锁应用。默认只填空字段；`force` 覆盖已有内容。
fn fetch_and_apply(paths: &Paths, id: i64, force: bool, scope: &Scope) -> Result<()> {
    let url = {
        let store = read_store(paths)?;
        let source = source_for_id(&store, id, scope).ok_or_else(|| scope.not_found(id))?;
        store.sources[&source]
            .iter()
            .find(|bookmark| bookmark.id == id)
            .map(|bookmark| bookmark.url.clone())
    }
    .ok_or_else(|| scope.not_found(id))?;

    let meta = fetcher::fetch(&url, Duration::from_secs(10))?;

    mutate(paths, |store| {
        let source = source_for_id(store, id, scope).ok_or_else(|| scope.not_found(id))?;
        let b = store
            .sources
            .get_mut(&source)
            .and_then(|bookmarks| bookmarks.iter_mut().find(|bookmark| bookmark.id == id))
            .ok_or_else(|| scope.not_found(id))?;
        let mut changed = false;
        // title 占位（== url）或空时才回填，避免覆盖用户已设标题（除非 --force）。
        if let Some(t) = meta.title
            && (force || b.title.is_empty() || b.title == b.url)
        {
            b.title = t;
            changed = true;
        }
        if let Some(e) = meta.excerpt
            && (force || b.excerpt.is_empty())
        {
            b.excerpt = e;
            changed = true;
        }
        if let Some(c) = meta.cover
            && (force || b.cover.is_empty())
        {
            b.cover = c;
            changed = true;
        }
        if changed {
            b.updated = now_millis();
        }
        Ok(())
    })
}

fn cmd_list(
    paths: &Paths,
    keyword: Option<String>,
    jq_filter: Option<String>,
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
        if let Some(filter) = &jq_filter {
            filtered = apply_jq_filter(&filtered, CURRENT_VERSION, filter)?;
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

/// 用内嵌 jq 引擎（jaq）过滤书签。输入 `{version, bookmarks}`（使 `.bookmarks[] | select(...)`
/// 等 §9 惯用过滤器可原样工作），输出须为书签对象；数组输出扁平化一层。
fn apply_jq_filter(bms: &[Bookmark], version: u32, jq: &str) -> Result<Vec<Bookmark>> {
    let input = serde_json::json!({ "version": version, "bookmarks": bms }).to_string();
    let outputs = filter::run_filter(&input, jq)?;
    let mut result = Vec::new();
    for v in outputs {
        // 兼容 `.bookmarks[] | ...`（对象流）与 `.bookmarks | map(...)` / `[...]`（数组）
        let items = match v {
            serde_json::Value::Array(arr) => arr,
            other => vec![other],
        };
        for item in items {
            let b: Bookmark = serde_json::from_value(item).map_err(|e| {
                anyhow!(
                    "--filter output is not a bookmark object ({e}). query only returns bookmarks; \
                     for arbitrary jq output, run jq directly against the data file."
                )
            })?;
            result.push(b);
        }
    }
    Ok(result)
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
    let n = mutate(paths, |store| {
        let prefix = format!("{old} / ");
        let now = now_millis();
        let mut moved = 0;
        for (source, bookmarks) in &mut store.sources {
            if !scope.includes_source(source) {
                continue;
            }
            let mut relocated: Vec<String> = Vec::new();
            for bookmark in bookmarks.iter_mut() {
                let new_folder = if bookmark.folder == old {
                    new.clone()
                } else if let Some(rest) = bookmark.folder.strip_prefix(&prefix) {
                    format!("{new} / {rest}")
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
            }
        }
        if moved == 0 {
            bail!("no folder matches {old:?}");
        }
        Ok(moved)
    })?;
    println!("Moved {n} bookmark(s): {old} → {new}");
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

/// `ancestor` 是否为 `descendant` 的严格前缀祖先（按 " / " 分段）。空路径不作祖先（未分类豁免）。
fn is_ancestor(ancestor: &str, descendant: &str) -> bool {
    !ancestor.is_empty() && descendant.starts_with(&format!("{ancestor} / "))
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
    use clap::{CommandFactory, error::ErrorKind};
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
    fn scope_is_root_only_and_defaults_to_default() {
        let cli = Cli::try_parse_from(["jj-bookmark", "ls"]).unwrap();
        assert_eq!(
            cli.scope.resolve(),
            Scope::Source(DEFAULT_SOURCE.to_owned())
        );

        let cli = Cli::try_parse_from(["jj-bookmark", "--source", "safari", "ls"]).unwrap();
        assert_eq!(cli.scope.resolve(), Scope::Source("safari".to_owned()));
        assert!(Cli::try_parse_from(["jj-bookmark", "ls", "--source", "safari"]).is_err());

        let command = Cli::command();
        assert!(command.get_arguments().any(|arg| arg.get_id() == "source"));
        assert!(command.get_arguments().any(|arg| arg.get_id() == "all"));
        let ls = command.find_subcommand("ls").unwrap();
        assert!(
            ls.get_arguments()
                .all(|arg| arg.get_id() != "source" && arg.get_id() != "all")
        );
    }

    #[test]
    fn source_and_all_are_mutually_exclusive() {
        let error = Cli::try_parse_from(["jj-bookmark", "--source", "safari", "--all", "ls"])
            .err()
            .expect("scope conflict must fail");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
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
        seed(&paths, DEFAULT_SOURCE, 1, "A / B / C"); // 现有叶子

        let add = |folder: Option<&str>| {
            cmd_add(
                &paths,
                "u".into(),
                None,
                folder.map(str::to_owned),
                None,
                None,
                false,
                default_scope(),
            )
        };
        // 挂到祖先 A / B → 拒绝
        assert!(add(Some("A / B")).is_err());
        // 挂到后代 A / B / C / D（会使 A/B/C 非叶）→ 拒绝
        assert!(add(Some("A / B / C / D")).is_err());
        // 兄弟叶子 A / B / E → 允许
        assert!(add(Some("A / B / E")).is_ok());
        // 同叶子再加一条 → 允许
        assert!(add(Some("A / B / C")).is_ok());
        // 未分类（空 folder）即使已有 folder 也允许（空 folder 豁免，策略 A）
        assert!(add(None).is_ok());
        let _ = fs::remove_dir_all(&paths.dir);
    }

    #[test]
    fn leaf_edit_rejects_non_leaf_but_allows_field_edit_on_dirty_folder() {
        let paths = temp_paths("leaf-edit");
        seed(&paths, DEFAULT_SOURCE, 1, "X / Y");
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
        // #2 → X（X 是 X/Y 的祖先，会非叶）→ 拒绝
        assert!(set_folder(2, "X").is_err());
        // #2 → 兄弟叶子 X / K → 允许
        assert!(set_folder(2, "X / K").is_ok());

        // 脏放置：P 与 P/Q 同时占用（绕过校验直接塞）
        seed(&paths, DEFAULT_SOURCE, 3, "P");
        seed(&paths, DEFAULT_SOURCE, 4, "P / Q");
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
        seed(&paths, DEFAULT_SOURCE, 2, "A / B"); // 目标 source 已有 A/B
        // #1 safari→default（folder 保持 A）→ default 里 A 成 A/B 祖先 → 拒绝
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
        seed(&paths, DEFAULT_SOURCE, 1, "A / B");
        seed(&paths, DEFAULT_SOURCE, 2, "A / C");
        // mv A/B → A/C：合并到同一叶子 → 允许
        assert!(cmd_mv(&paths, "A / B".into(), "A / C".into(), default_scope()).is_ok());
        let _ = fs::remove_dir_all(&paths.dir);

        let paths = temp_paths("leaf-mv-bad");
        seed(&paths, DEFAULT_SOURCE, 1, "A / B");
        seed(&paths, DEFAULT_SOURCE, 2, "C");
        // mv A/B → C/D：C 已占用且会成 C/D 祖先 → 拒绝
        assert!(cmd_mv(&paths, "A / B".into(), "C / D".into(), default_scope()).is_err());
        let _ = fs::remove_dir_all(&paths.dir);
    }
}
