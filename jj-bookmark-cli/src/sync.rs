//! sync：本地文件 ⇄ Cloudflare（Worker + R2）双向同步（data-model §12）。
//!
//! 协议：`GET` 取远端全量 + `ETag` → 与本地合并 → `PUT` 带 `If-Match` 条件写。
//! 远端在此期间被改动 → Worker 回 409 → 重新 GET 再来一轮（合并幂等，重跑不会出错）。
//!
//! 合并 = **全局 id map → 记录级 LWW（比 `updated`）→ `last_visited` 取 max → 按胜者的 source
//! 重新分组**。三条规则都幂等且可交换，故两端反复 sync 必然收敛。删除靠 `deleted` 墓碑表达，
//! 没有墓碑，「一端删、另一端未删」会让已删条目复活。
//!
//! HTTP 经系统 `curl`（macOS 自带），不引入 TLS 依赖链——universal 交叉编译零风险。
//! 参数走 `--config -`（stdin），故 service token **绝不出现在 argv / `ps` 里**。

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet, btree_map::Entry};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::model::{Bookmark, Store};
use crate::query::is_ancestor;
use crate::store::{Paths, mutate_capture};

/// 同步端点：与 Worker 路由 `/api/bookmarks` 逐字对齐。单人单库，写死最简、最不易漂移。
pub const ENDPOINT: &str = "https://jj-bookmark.yigegongjiang.com/api/bookmarks";
/// 409（远端在 GET 与 PUT 之间被改）重试上限。单人使用几乎不会用到第 2 轮。
const MAX_ATTEMPTS: u32 = 5;
/// 单次 HTTP 超时（秒）。数据不到 1 MB，超过这个量级即为网络异常而非慢。
const TIMEOUT_SECS: u32 = 60;

// ---- 合并 ----

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MergeStats {
    /// 远端胜出（含仅远端有）的条数 = 本次被拉下来的改动。
    pub from_remote: usize,
    /// 本地胜出（含仅本地有）的条数 = 本次将被推上去的改动。
    pub to_remote: usize,
    /// 合并后总条数（含墓碑）。
    pub total: usize,
}

/// 把 `remote` 合并进 `local`（原地）。见模块文档的三条规则。
///
/// `updated` 相等 → 保留本地（平局偏本地，避免无谓写放大）；两端同内容时这是常态。
pub fn merge_into(local: &mut Store, remote: &Store) -> MergeStats {
    let mut merged: BTreeMap<i64, (String, Bookmark)> = local
        .iter_with_source()
        .map(|(source, b)| (b.id, (source.to_owned(), b.clone())))
        .collect();
    let mut stats = MergeStats::default();

    for (rsource, r) in remote.iter_with_source() {
        match merged.entry(r.id) {
            Entry::Vacant(slot) => {
                stats.from_remote += 1;
                slot.insert((rsource.to_owned(), r.clone()));
            }
            Entry::Occupied(mut slot) => {
                let (lsource, l) = slot.get_mut();
                // 访问时刻不是内容修改（data-model §10），故不跟随 LWW 胜者，单独取 max，
                // 否则「web 改标题 + 本地刚打开过」会把访问时间抹回去，破坏「最近访问」排序。
                let visited = l.last_visited.max(r.last_visited);
                match r.updated.cmp(&l.updated) {
                    Ordering::Greater => {
                        stats.from_remote += 1;
                        *lsource = rsource.to_owned();
                        *l = r.clone();
                    }
                    Ordering::Less => stats.to_remote += 1,
                    Ordering::Equal => {}
                }
                l.last_visited = visited;
            }
        }
    }

    let remote_ids: HashSet<i64> = remote.iter_with_source().map(|(_, b)| b.id).collect();
    stats.to_remote += merged.keys().filter(|id| !remote_ids.contains(id)).count();
    stats.total = merged.len();

    // 按胜者的 source 重新分组。BTreeMap 按 id 升序迭代 → 文件顺序确定（数组顺序无语义）。
    local.sources.clear();
    for (_, (source, bookmark)) in merged {
        local.sources.entry(source).or_default().push(bookmark);
    }
    stats
}

/// 合并后可能违反叶子挂载约束（data-model §5）：两端各自合法的放置，合到一起可能互为祖先。
/// 这是已接受的妥协——只报告，不阻断同步，也不回滚（阻断会让两端永远同步不上）。
/// 返回 `(source, 祖先, 后代)` 三元组，墓碑不计入（已删条目不占 folder）。
pub fn leaf_violations(store: &Store) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    for (source, bookmarks) in &store.sources {
        let mut folders: Vec<&str> = bookmarks
            .iter()
            .filter(|b| !b.deleted && !b.folder.is_empty())
            .map(|b| b.folder.as_str())
            .collect();
        folders.sort_unstable();
        folders.dedup();
        for (i, ancestor) in folders.iter().enumerate() {
            for descendant in &folders[i + 1..] {
                if is_ancestor(ancestor, descendant) {
                    found.push((source.clone(), (*ancestor).to_owned(), (*descendant).to_owned()));
                }
            }
        }
    }
    found
}

// ---- 同步流程 ----

/// 拉取 → 合并 → 条件写。409（远端被改）自动重来，上限 [`MAX_ATTEMPTS`]。
pub fn sync(paths: &Paths) -> Result<()> {
    let credentials = Credentials::load(paths)?;

    for attempt in 1..=MAX_ATTEMPTS {
        let (etag, remote) = fetch_remote(&credentials)?;
        let (outcome_of_merge, bytes) = mutate_capture(paths, |local| {
            let stats = merge_into(local, &remote);
            Ok((stats, leaf_violations(local)))
        })?;

        write_upload_body(paths, &bytes)?;
        let outcome = put_remote(&credentials, etag.as_deref(), &paths.sync);
        let _ = fs::remove_file(&paths.sync); // 无论成败都不留残留文件

        match outcome? {
            PutOutcome::Stored => {
                let (stats, violations) = outcome_of_merge;
                report(stats, &violations);
                return Ok(());
            }
            PutOutcome::Conflict => eprintln!(
                "remote changed mid-sync; merging again ({attempt}/{MAX_ATTEMPTS})"
            ),
        }
    }
    bail!(
        "sync gave up after {MAX_ATTEMPTS} attempts: the remote kept changing between GET and PUT. \
         Local data is already merged and safe; just run `jj-bookmark sync` again."
    )
}

fn report(stats: MergeStats, violations: &[(String, String, String)]) {
    println!(
        "Synced: {} pulled, {} pushed, {} total (including tombstones)",
        stats.from_remote, stats.to_remote, stats.total
    );
    // 合并可能把两端各自合法的 folder 放置合成非叶挂载；只提示，不阻断（见 leaf_violations）。
    for (source, ancestor, descendant) in violations {
        eprintln!(
            "warning: source {source:?} holds bookmarks in both {ancestor:?} and its descendant \
             {descendant:?}; fix with `jj-bookmark --source {source} mv`"
        );
    }
}

/// 写上传体到独立临时文件（0600）。
///
/// 不上传 `bookmarks.json` 本身：原子写用 rename 换 inode，curl 可能读到 merge 之前的旧内容，
/// 那样记下的 etag 就对应一份从未校验过的远端状态。
fn write_upload_body(paths: &Paths, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&paths.sync)
        .with_context(|| format!("failed to create upload file: {}", paths.sync.display()))?;
    file.write_all(bytes).context("failed to write upload file")?;
    file.sync_all().context("failed to fsync upload file")?;
    Ok(())
}

// ---- Cloudflare Access service token ----

/// `~/.config/jj-bookmark/credentials.json`，权限必须 0600（内含可读全库的密钥）。
#[derive(Deserialize)]
pub struct Credentials {
    client_id: String,
    client_secret: String,
}

impl Credentials {
    fn load(paths: &Paths) -> Result<Credentials> {
        let path = &paths.credentials;
        if !path.exists() {
            // 第 2 步是最容易漏的一步：只建 token 而不给应用加 Service Auth 策略，
            // Access 会当成浏览器访问、302 去登录页，报错看起来完全不像「少了策略」。
            bail!(
                "missing {path}. One-time setup — two clicks in Cloudflare, then one command:\n\
                 \n  1. Zero Trust → Access → Service auth → Service Tokens → Create Service Token\
                 \n  2. Zero Trust → Access → Applications → the app serving {host} → Policies →\
                 \n     add a policy with action `Service Auth` that accepts that token\
                 \n     (skip this and Access asks for a browser login; sync then fails with HTTP 302)\
                 \n  3. printf '{{\"client_id\":\"<ID>\",\"client_secret\":\"<SECRET>\"}}' > {path} \\\
                 \n       && chmod 600 {path}",
                path = path.display(),
                host = ENDPOINT
                    .trim_start_matches("https://")
                    .split('/')
                    .next()
                    .unwrap_or(ENDPOINT),
            );
        }
        let mode = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            bail!(
                "{} is group/world readable (mode {:o}); it holds a token with full read/write \
                 access to your bookmarks. Run: chmod 600 {}",
                path.display(),
                mode & 0o777,
                path.display()
            );
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let credentials: Credentials = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {} (expected {{client_id, client_secret}})", path.display()))?;
        for (field, value) in [
            ("client_id", &credentials.client_id),
            ("client_secret", &credentials.client_secret),
        ] {
            if value.is_empty() {
                bail!("{} has an empty {field}", path.display());
            }
            // curl 配置是逐行解析的：控制字符会截断行、把后续内容当成新指令。
            if value.contains(|c: char| c.is_control()) {
                bail!("{} field {field} contains control characters", path.display());
            }
        }
        Ok(credentials)
    }
}

// ---- HTTP（经系统 curl） ----

struct HttpResponse {
    status: u16,
    etag: Option<String>,
    body: String,
}

enum PutOutcome {
    Stored,
    Conflict,
}

fn fetch_remote(credentials: &Credentials) -> Result<(Option<String>, Store)> {
    let response = run_curl(&base_config(credentials))?;
    match response.status {
        200 => {
            let store: Store = serde_json::from_str(&response.body)
                .context("remote returned invalid bookmark JSON")?;
            Ok((response.etag, store))
        }
        status => Err(http_error("GET", status, &response.body)),
    }
}

fn put_remote(credentials: &Credentials, etag: Option<&str>, file: &Path) -> Result<PutOutcome> {
    let path = file.to_str().context("upload file path is not valid UTF-8")?;
    let mut config = base_config(credentials);
    config.push_str("request = \"PUT\"\n");
    config.push_str("header = \"Content-Type: application/json\"\n");
    if let Some(etag) = etag {
        config.push_str(&format!("header = \"If-Match: {}\"\n", escape(etag)));
    }
    config.push_str(&format!("upload-file = \"{}\"\n", escape(path)));

    let response = run_curl(&config)?;
    match response.status {
        200 | 201 | 204 => Ok(PutOutcome::Stored),
        409 => Ok(PutOutcome::Conflict),
        status => Err(http_error("PUT", status, &response.body)),
    }
}

fn http_error(method: &str, status: u16, body: &str) -> anyhow::Error {
    let hint = match status {
        // Access 未放行：token 无效 / 应用缺 Service Auth 策略时会 302 去登录页。
        301..=399 | 401 | 403 => {
            " — Access rejected the service token. Check credentials.json and that the Access \
             application has a policy with action `Service Auth` covering this token."
        }
        413 => " — payload too large for the Worker limit.",
        _ => "",
    };
    anyhow::anyhow!("{method} {ENDPOINT} failed: HTTP {status}{hint}\n{}", body.trim())
}

/// 公共 curl 配置。`--config -` 从 stdin 读，故密钥不进 argv（`ps` 看不到）。
/// 不加 `location`：Access 拒绝时会 302 到登录页，跟随重定向会把它伪装成 200 HTML。
fn base_config(credentials: &Credentials) -> String {
    format!(
        "url = \"{}\"\n\
         header = \"CF-Access-Client-Id: {}\"\n\
         header = \"CF-Access-Client-Secret: {}\"\n\
         max-time = {TIMEOUT_SECS}\n\
         silent\n\
         show-error\n\
         write-out = \"\\n%{{http_code}}\\n%header{{etag}}\"\n",
        escape(ENDPOINT),
        escape(&credentials.client_id),
        escape(&credentials.client_secret),
    )
}

/// curl 配置里双引号值只认 `\\` 与 `\"` 两种转义（控制字符已在 Credentials::load 挡掉）。
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn run_curl(config: &str) -> Result<HttpResponse> {
    let mut child = Command::new("curl")
        .arg("--config")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to run `curl` (macOS ships it; is PATH broken?)")?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(config.as_bytes())
        .context("failed to hand the curl config to stdin")?;
    let output = child.wait_with_output().context("curl did not finish")?;
    if !output.status.success() {
        // 未加 --fail，故 HTTP 4xx/5xx 仍是 exit 0；非零 = 网络 / TLS / 超时。
        bail!(
            "curl exited {} (network, TLS, or timeout error)",
            output.status.code().unwrap_or(-1)
        );
    }
    parse_response(&String::from_utf8_lossy(&output.stdout))
}

/// `--write-out` 在响应体后追加 `\n<http_code>\n<etag>`，故恒从**末尾**切两行。
/// 409 的响应体是纯文本、etag 可能为空，所以不能假设正文是 JSON。
fn parse_response(raw: &str) -> Result<HttpResponse> {
    let mut parts = raw.rsplitn(3, '\n');
    let etag = parts.next().unwrap_or_default();
    let status = parts
        .next()
        .context("curl produced no status line (write-out format broken?)")?;
    let body = parts.next().unwrap_or_default();
    Ok(HttpResponse {
        status: status
            .trim()
            .parse()
            .with_context(|| format!("unparsable HTTP status from curl: {status:?}"))?,
        etag: normalize_etag(etag),
        body: body.to_owned(),
    })
}

/// HTTP 头里的 etag 带引号、可能带弱校验前缀 `W/`；R2 的 `etagMatches` 要裸值。
fn normalize_etag(raw: &str) -> Option<String> {
    let value = raw.trim().trim_start_matches("W/").trim_matches('"');
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CURRENT_VERSION;

    fn store(entries: &[(&str, i64, &str, i64, bool)]) -> Store {
        // (source, id, title, updated, deleted)
        let mut store = Store::default();
        for (source, id, title, updated, deleted) in entries {
            let mut bookmark = Bookmark::new(*id, "u".into(), (*title).into(), "".into(), "".into());
            bookmark.updated = *updated;
            bookmark.deleted = *deleted;
            store
                .sources
                .entry((*source).to_owned())
                .or_default()
                .push(bookmark);
        }
        store
    }

    /// 合并后的可比较快照：(source, id, title, updated, deleted, last_visited)。
    fn snapshot(store: &Store) -> Vec<(String, i64, String, i64, bool, i64)> {
        let mut rows: Vec<_> = store
            .iter_with_source()
            .map(|(source, b)| {
                (
                    source.to_owned(),
                    b.id,
                    b.title.clone(),
                    b.updated,
                    b.deleted,
                    b.last_visited,
                )
            })
            .collect();
        rows.sort();
        rows
    }

    #[test]
    fn disjoint_adds_are_both_kept() {
        let mut local = store(&[("default", 1, "local", 10, false)]);
        let remote = store(&[("default", 2, "remote", 10, false)]);
        let stats = merge_into(&mut local, &remote);
        assert_eq!(
            stats,
            MergeStats { from_remote: 1, to_remote: 1, total: 2 }
        );
        assert_eq!(local.sources["default"].len(), 2);
    }

    #[test]
    fn newer_updated_wins_in_both_directions() {
        let mut local = store(&[("default", 1, "local-new", 20, false)]);
        merge_into(&mut local, &store(&[("default", 1, "remote-old", 10, false)]));
        assert_eq!(local.sources["default"][0].title, "local-new");

        let mut local = store(&[("default", 1, "local-old", 10, false)]);
        merge_into(&mut local, &store(&[("default", 1, "remote-new", 20, false)]));
        assert_eq!(local.sources["default"][0].title, "remote-new");
    }

    #[test]
    fn equal_updated_keeps_local_and_counts_nothing() {
        let mut local = store(&[("default", 1, "local", 10, false)]);
        let stats = merge_into(&mut local, &store(&[("default", 1, "remote", 10, false)]));
        assert_eq!(local.sources["default"][0].title, "local");
        assert_eq!(stats, MergeStats { from_remote: 0, to_remote: 0, total: 1 });
    }

    #[test]
    fn delete_versus_edit_is_decided_by_timestamp() {
        // 远端删得更晚 → 删除生效
        let mut local = store(&[("default", 1, "edited", 10, false)]);
        merge_into(&mut local, &store(&[("default", 1, "edited", 20, true)]));
        assert!(local.sources["default"][0].deleted);

        // 本地改得更晚 → 记录保留（墓碑输掉）
        let mut local = store(&[("default", 1, "edited-later", 30, false)]);
        merge_into(&mut local, &store(&[("default", 1, "gone", 20, true)]));
        assert!(!local.sources["default"][0].deleted);
    }

    #[test]
    fn tombstone_prevents_resurrection() {
        // 本地已删；远端根本没有这条（例如远端从未见过它）→ 墓碑必须留下，
        // 否则下一轮远端把「没有」当成「未删」，条目就复活了。
        let mut local = store(&[("default", 1, "gone", 20, true)]);
        merge_into(&mut local, &Store::default());
        assert_eq!(local.sources["default"].len(), 1);
        assert!(local.sources["default"][0].deleted);
    }

    #[test]
    fn last_visited_takes_the_max_regardless_of_the_lww_winner() {
        let mut local = store(&[("default", 1, "local", 10, false)]);
        local.sources.get_mut("default").unwrap()[0].last_visited = 999;
        let mut remote = store(&[("default", 1, "remote-wins", 20, false)]);
        remote.sources.get_mut("default").unwrap()[0].last_visited = 5;

        merge_into(&mut local, &remote);
        let merged = &local.sources["default"][0];
        assert_eq!(merged.title, "remote-wins", "内容按 LWW");
        assert_eq!(merged.last_visited, 999, "访问时刻取 max，不跟随胜者");
    }

    #[test]
    fn winner_decides_the_source_so_moves_do_not_duplicate() {
        // 同一条在本地是 safari、远端已被移到 default 且更新更晚 → 只应剩一条，落在 default。
        let mut local = store(&[("safari", 1, "old", 10, false)]);
        merge_into(&mut local, &store(&[("default", 1, "moved", 20, false)]));
        assert_eq!(local.sources.len(), 1);
        assert_eq!(local.sources["default"][0].title, "moved");
        assert!(!local.sources.contains_key("safari"));
    }

    #[test]
    fn merge_is_idempotent_and_commutative() {
        let a = store(&[
            ("default", 1, "a1", 30, false),
            ("safari", 2, "a2", 10, false),
            ("default", 4, "a4", 40, true),
        ]);
        let b = store(&[
            ("default", 1, "b1", 20, false),
            ("safari", 2, "b2", 50, false),
            ("default", 3, "b3", 10, false),
        ]);

        let mut ab = a.clone();
        merge_into(&mut ab, &b);
        let mut ba = b.clone();
        merge_into(&mut ba, &a);
        assert_eq!(snapshot(&ab), snapshot(&ba), "两端合并结果必须一致 → 收敛");

        let mut twice = ab.clone();
        merge_into(&mut twice, &b);
        assert_eq!(snapshot(&twice), snapshot(&ab), "重复合并不改变结果 → 幂等");
    }

    #[test]
    fn leaf_violations_reports_only_live_ancestor_pairs() {
        let mut store = Store::default();
        let mut push = |source: &str, id: i64, folder: &str, deleted: bool| {
            let mut b = Bookmark::new(id, "u".into(), "t".into(), folder.into(), "".into());
            b.deleted = deleted;
            store.sources.entry(source.to_owned()).or_default().push(b);
        };
        push("default", 1, "A", false);
        push("default", 2, "A::B", false);
        push("default", 3, "", false); // 未分类豁免
        push("safari", 4, "X", false);
        push("safari", 5, "X::Y", true); // 墓碑不占 folder

        let violations = leaf_violations(&store);
        assert_eq!(
            violations,
            vec![("default".to_owned(), "A".to_owned(), "A::B".to_owned())]
        );
    }

    // ---- HTTP 解析 ----

    #[test]
    fn parse_response_splits_body_status_and_etag_from_the_end() {
        let response = parse_response("{\"version\":4,\n\"sources\":{}}\n200\n\"abc\"").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.etag.as_deref(), Some("abc"));
        assert_eq!(response.body, "{\"version\":4,\n\"sources\":{}}");
    }

    #[test]
    fn parse_response_handles_plain_text_body_and_missing_etag() {
        // 409 的响应体是纯文本、etag 头缺失 → 解析不得假设正文是 JSON
        let response = parse_response("etag mismatch\n\n409\n").unwrap();
        assert_eq!(response.status, 409);
        assert_eq!(response.etag, None);
        assert_eq!(response.body, "etag mismatch\n");
    }

    #[test]
    fn etag_is_normalized_to_the_bare_value() {
        assert_eq!(normalize_etag("\"abc\""), Some("abc".into()));
        assert_eq!(normalize_etag("W/\"abc\""), Some("abc".into()));
        assert_eq!(normalize_etag("  "), None);
        assert_eq!(normalize_etag(""), None);
    }

    #[test]
    fn curl_config_values_escape_quotes_and_backslashes() {
        assert_eq!(escape(r#"a"b\c"#), r#"a\"b\\c"#);
        let config = base_config(&Credentials {
            client_id: r#"id"x"#.into(),
            client_secret: "secret".into(),
        });
        assert!(config.contains(r#"header = "CF-Access-Client-Id: id\"x""#));
        assert!(config.contains(&format!("url = \"{ENDPOINT}\"")));
        // 不跟随重定向：Access 拒绝时的 302 必须原样暴露，不能被伪装成 200 登录页
        assert!(!config.contains("location"));
    }

    /// 守住生产端点：本地联调时改过 `ENDPOINT` 忘了还原，会静默把数据同步到别处。
    #[test]
    fn endpoint_points_at_production() {
        assert_eq!(ENDPOINT, "https://jj-bookmark.yigegongjiang.com/api/bookmarks");
    }

    #[test]
    fn schema_version_is_v4() {
        assert_eq!(CURRENT_VERSION, 4);
    }
}
