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
use clap::{Parser, Subcommand};
use std::time::Duration;

use model::{Bookmark, Store, now_millis};
use query::{Order, SortKey};
use store::{Paths, mutate, read_store};

#[derive(Parser)]
#[command(
    name = "jj-bookmark",
    version,
    about = "Bookmark tool",
    before_help = "TL;DR — add a bookmark:\n  Existing folder paths: jj-bookmark folders\n  jj-bookmark add <URL> [--folder <PATH>] [--title <TITLE>] [--note <NOTE>] [--fetch]\n  Only URL is required; omit --folder for uncategorized; --fetch retrieves metadata now."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a bookmark
    Add {
        /// Target URL
        url: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        note: Option<String>,
        /// Fetch metadata synchronously now (off by default to avoid blocking; the App fetches in the background)
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
    /// Edit fields
    Edit {
        id: i64,
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
    },
    /// Delete a bookmark
    Rm { id: i64 },
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
    match cli.cmd {
        Command::Add { url, title, folder, note, fetch } => {
            cmd_add(&paths, url, title, folder, note, fetch)
        }
        Command::Ls { folder, sort, order, json } => {
            cmd_list(&paths, None, None, folder, sort, order, json)
        }
        Command::Folders => cmd_folders(&paths),
        Command::Query { keyword, filter, folder, sort, order, json } => {
            cmd_list(&paths, Some(keyword), filter, folder, sort, order, json)
        }
        Command::Open { id } => cmd_open(&paths, id),
        Command::Edit { id, title, url, folder, note, excerpt } => {
            cmd_edit(&paths, id, title, url, folder, note, excerpt)
        }
        Command::Rm { id } => cmd_rm(&paths, id),
        Command::Fetch { id, force } => cmd_fetch(&paths, id, force),
        Command::Mv { old, new } => cmd_mv(&paths, old, new),
        Command::Push => cmd_push(&paths),
    }
}

fn cmd_add(
    paths: &Paths,
    url: String,
    title: Option<String>,
    folder: Option<String>,
    note: Option<String>,
    fetch: bool,
) -> Result<()> {
    let title = title.unwrap_or_else(|| url.clone()); // 无 --title 时用 URL 占位；抓取会回填
    let folder = folder.unwrap_or_default();
    let note = note.unwrap_or_default();
    let id = mutate(paths, |store| {
        let id = unique_id(store);
        store
            .bookmarks
            .push(Bookmark::new(id, url.clone(), title.clone(), folder.clone(), note.clone()));
        Ok(id)
    })?;
    println!("Added #{id}");
    if fetch {
        // 同步抓取；失败不回滚（书签已保存），仅告警。
        if let Err(e) = fetch_and_apply(paths, id, false) {
            eprintln!("Warning: metadata fetch failed (bookmark saved): {e:#}");
        }
    }
    Ok(())
}

fn cmd_folders(paths: &Paths) -> Result<()> {
    let mut folders: Vec<_> = read_store(paths)?
        .bookmarks
        .into_iter()
        .map(|b| b.folder)
        .filter(|folder| !folder.is_empty())
        .collect();
    folders.sort();
    folders.dedup();
    for folder in folders {
        println!("{folder}");
    }
    Ok(())
}

fn cmd_fetch(paths: &Paths, id: i64, force: bool) -> Result<()> {
    fetch_and_apply(paths, id, force)?;
    println!("Fetched #{id}");
    Ok(())
}

/// 抓取并回填元数据。网络请求在写锁**之外**进行（避免长时间持锁阻塞其他写），
/// 抓取完成后再加锁应用。默认只填空字段；`force` 覆盖已有内容。
fn fetch_and_apply(paths: &Paths, id: i64, force: bool) -> Result<()> {
    let url = {
        let store = read_store(paths)?;
        store.bookmarks.iter().find(|b| b.id == id).map(|b| b.url.clone())
    }
    .ok_or_else(|| anyhow!("bookmark #{id} not found"))?;

    let meta = fetcher::fetch(&url, Duration::from_secs(10))?;

    mutate(paths, |store| {
        let b = store.find_mut(id).ok_or_else(|| anyhow!("bookmark #{id} not found"))?;
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
) -> Result<()> {
    let store = read_store(paths)?;
    let mut bms = store.bookmarks;
    if let Some(kw) = keyword {
        bms = query::keyword_filter(bms, &kw);
    }
    if let Some(f) = folder {
        bms = query::folder_filter(bms, &f);
    }
    if let Some(f) = jq_filter {
        bms = apply_jq_filter(&bms, store.version, &f)?;
    }
    let order = order.unwrap_or_else(|| sort.default_order());
    query::sort_bookmarks(&mut bms, sort, order);
    if json {
        output::print_json(&bms, store.version)?;
    } else {
        output::print_human(&bms);
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

fn cmd_open(paths: &Paths, id: i64) -> Result<()> {
    let url = mutate(paths, |store| {
        let b = store.find_mut(id).ok_or_else(|| anyhow!("bookmark #{id} not found"))?;
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
) -> Result<()> {
    if title.is_none() && url.is_none() && folder.is_none() && note.is_none() && excerpt.is_none() {
        bail!("no fields provided to modify (--title/--url/--folder/--note/--excerpt)");
    }
    mutate(paths, |store| {
        let b = store.find_mut(id).ok_or_else(|| anyhow!("bookmark #{id} not found"))?;
        if let Some(t) = title {
            b.title = t;
        }
        if let Some(u) = url {
            b.url = u;
        }
        if let Some(f) = folder {
            b.folder = f;
        }
        if let Some(n) = note {
            b.note = n;
        }
        if let Some(e) = excerpt {
            b.excerpt = e;
        }
        b.updated = now_millis(); // 内容修改 → 更新 updated
        Ok(())
    })?;
    println!("Updated #{id}");
    Ok(())
}

fn cmd_rm(paths: &Paths, id: i64) -> Result<()> {
    mutate(paths, |store| {
        let before = store.bookmarks.len();
        store.bookmarks.retain(|b| b.id != id);
        if store.bookmarks.len() == before {
            bail!("bookmark #{id} not found"); // 在锁内报错 → 不产生无谓写
        }
        Ok(())
    })?;
    println!("Deleted #{id}");
    Ok(())
}

fn cmd_mv(paths: &Paths, old: String, new: String) -> Result<()> {
    let n = mutate(paths, |store| {
        let prefix = format!("{old} / ");
        let now = now_millis();
        let mut moved = 0;
        for b in &mut store.bookmarks {
            if b.folder == old {
                b.folder = new.clone();
            } else if let Some(rest) = b.folder.strip_prefix(&prefix) {
                b.folder = format!("{new} / {rest}"); // 子树前缀替换
            } else {
                continue;
            }
            b.updated = now; // folder 变更 = 内容修改
            moved += 1;
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
    println!("Pushing {} → R2 {}/{}", paths.data.display(), pusher::BUCKET, pusher::KEY);
    pusher::push(paths)?;
    println!("Pushed to R2: {}/{}", pusher::BUCKET, pusher::KEY);
    Ok(())
}

/// 生成唯一 id：Unix 毫秒；极端同毫秒冲突则 +1 重试（data-model §7）。
fn unique_id(store: &Store) -> i64 {
    let mut id = now_millis();
    while store.bookmarks.iter().any(|b| b.id == id) {
        id += 1;
    }
    id
}
