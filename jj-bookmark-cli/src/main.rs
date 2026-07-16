//! jj-bookmark CLI — 唯一核心。读写协议 / 查询 / 导入 / 抓取只在此实现一遍，
//! App 经内嵌调用复用。命令见 roadmap.md，数据契约见 data-model.md。

mod fetcher;
mod filter;
mod importer;
mod model;
mod output;
mod query;
mod store;
mod timeutil;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use model::{Bookmark, Store, now_millis};
use query::{Order, SortKey};
use store::{Paths, mutate, read_store};

#[derive(Parser)]
#[command(name = "jj-bookmark", version, about = "极简本地书签工具（核心 CLI）")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 新增书签
    Add {
        /// 目标 URL
        url: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        note: Option<String>,
        /// 立即同步抓取元数据（默认不抓以免阻塞；App 用后台 fetch）
        #[arg(long)]
        fetch: bool,
    },
    /// 列出书签（支持排序）
    Ls {
        #[arg(long)]
        folder: Option<String>,
        #[arg(long, value_enum, default_value_t = SortKey::Created)]
        sort: SortKey,
        #[arg(long, value_enum)]
        order: Option<Order>,
        /// 输出 --json 契约（App/脚本消费）
        #[arg(long)]
        json: bool,
    },
    /// 模糊搜索 title/url/excerpt（支持排序）；`--filter` 收原生 jq 过滤器（内嵌 jaq 执行）
    Query {
        /// 关键词（可为空字符串以匹配全部，配合 --filter 使用）
        keyword: String,
        /// 原生 jq 过滤器；输入为 {version, bookmarks}，输出须为书签对象（如 `.bookmarks[] | select(.favorite)`）
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
    /// 用默认浏览器打开 URL 并记录最近访问
    Open { id: i64 },
    /// 编辑字段
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
    /// 删除书签
    Rm { id: i64 },
    /// 抓取并回填元数据（title/excerpt/cover）
    Fetch {
        id: i64,
        /// 覆盖已有字段（默认只填空字段，不覆盖用户内容）
        #[arg(long)]
        force: bool,
    },
    /// 重命名 / 移动 folder 子树（前缀替换所有匹配项，单次原子写）
    Mv {
        /// 旧 folder 路径（含其所有后代）
        old: String,
        /// 新 folder 路径
        new: String,
    },
    /// 从 raindrop CSV 导入
    Import { csv: PathBuf },
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
        Command::Import { csv } => cmd_import(&paths, &csv),
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
    println!("已添加 #{id}");
    if fetch {
        // 同步抓取；失败不回滚（书签已保存），仅告警。
        if let Err(e) = fetch_and_apply(paths, id, false) {
            eprintln!("警告：元数据抓取失败（书签已保存）：{e:#}");
        }
    }
    Ok(())
}

fn cmd_fetch(paths: &Paths, id: i64, force: bool) -> Result<()> {
    fetch_and_apply(paths, id, force)?;
    println!("已抓取 #{id}");
    Ok(())
}

/// 抓取并回填元数据。网络请求在写锁**之外**进行（避免长时间持锁阻塞其他写），
/// 抓取完成后再加锁应用。默认只填空字段；`force` 覆盖已有内容。
fn fetch_and_apply(paths: &Paths, id: i64, force: bool) -> Result<()> {
    let url = {
        let store = read_store(paths)?;
        store.bookmarks.iter().find(|b| b.id == id).map(|b| b.url.clone())
    }
    .ok_or_else(|| anyhow!("找不到书签 #{id}"))?;

    let meta = fetcher::fetch(&url, Duration::from_secs(10))?;

    mutate(paths, |store| {
        let b = store.find_mut(id).ok_or_else(|| anyhow!("找不到书签 #{id}"))?;
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
                    "--filter 的输出不是书签对象（{e}）。query 只返回书签；\
                     如需任意 jq 输出，请直接对数据文件运行 jq。"
                )
            })?;
            result.push(b);
        }
    }
    Ok(result)
}

fn cmd_open(paths: &Paths, id: i64) -> Result<()> {
    let url = mutate(paths, |store| {
        let b = store.find_mut(id).ok_or_else(|| anyhow!("找不到书签 #{id}"))?;
        b.last_visited = now_millis(); // 记录访问；不改 updated（访问 ≠ 内容修改）
        Ok(b.url.clone())
    })?;
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .with_context(|| format!("调用系统 open 失败: {url}"))?;
    println!("已打开 #{id}: {url}");
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
        bail!("未提供任何要修改的字段（--title/--url/--folder/--note/--excerpt）");
    }
    mutate(paths, |store| {
        let b = store.find_mut(id).ok_or_else(|| anyhow!("找不到书签 #{id}"))?;
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
    println!("已更新 #{id}");
    Ok(())
}

fn cmd_rm(paths: &Paths, id: i64) -> Result<()> {
    mutate(paths, |store| {
        let before = store.bookmarks.len();
        store.bookmarks.retain(|b| b.id != id);
        if store.bookmarks.len() == before {
            bail!("找不到书签 #{id}"); // 在锁内报错 → 不产生无谓写
        }
        Ok(())
    })?;
    println!("已删除 #{id}");
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
            bail!("没有 folder 匹配 {old:?}");
        }
        Ok(moved)
    })?;
    println!("已移动 {n} 条：{old} → {new}");
    Ok(())
}

fn cmd_import(paths: &Paths, csv: &Path) -> Result<()> {
    let incoming = importer::parse_raindrop_csv(csv)?;
    let total = incoming.len();
    let (imported, skipped) = mutate(paths, |store| {
        let mut seen: HashSet<i64> = store.bookmarks.iter().map(|b| b.id).collect();
        let (mut imported, mut skipped) = (0usize, 0usize);
        for b in incoming {
            if seen.insert(b.id) {
                store.bookmarks.push(b);
                imported += 1;
            } else {
                skipped += 1; // id 已存在（库内或 CSV 内重复）
            }
        }
        Ok((imported, skipped))
    })?;
    println!("导入完成：解析 {total} 条，新增 {imported}，跳过 {skipped}（id 已存在）");
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
