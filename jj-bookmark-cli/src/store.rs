//! 读写协议（data-model §6，唯一实现处）。
//!
//! 核心正确性原则：**原子 rename 会更换 inode**，故互斥锁绝不能绑定数据文件本身，
//! 必须用永不被 rename 的独立锁文件 `bookmarks.json.lock`。写严格按
//! 「加锁 → 从磁盘重读 → 改 → 写 tmp → fsync → 备份 → rename → 解锁」，
//! 这样并发的另一个写进程会被「锁 + 重读」吸收，不丢更新。

use anyhow::{Context, Result, anyhow, bail};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use crate::model::{CURRENT_VERSION, Store};

/// 数据目录下的一组路径。
pub struct Paths {
    pub dir: PathBuf,
    pub data: PathBuf,
    pub lock: PathBuf,
    pub bak: PathBuf,
    pub tmp: PathBuf,
}

impl Paths {
    /// 解析数据目录：优先 `JJ_BOOKMARK_DIR`（便于测试隔离），否则 `~/.config/jj-bookmark`。
    pub fn resolve() -> Result<Paths> {
        let dir = match std::env::var_os("JJ_BOOKMARK_DIR") {
            Some(d) => PathBuf::from(d),
            None => {
                let home = std::env::var_os("HOME").context("环境变量 HOME 未设置")?;
                PathBuf::from(home).join(".config").join("jj-bookmark")
            }
        };
        Ok(Paths::from_dir(dir))
    }

    pub fn from_dir(dir: PathBuf) -> Paths {
        Paths {
            data: dir.join("bookmarks.json"),
            lock: dir.join("bookmarks.json.lock"),
            bak: dir.join("bookmarks.json.bak"),
            tmp: dir.join("bookmarks.json.tmp"),
            dir,
        }
    }
}

/// 容错读：文件不存在 → 空库；解析失败 → 报错并保留原文件；version 过高 → 拒绝。
/// 读侧无需加锁：原子 rename 保证永远读到某个完整版本，不会是半写文件。
pub fn read_store(paths: &Paths) -> Result<Store> {
    if !paths.data.exists() {
        return Ok(Store::default());
    }
    let bytes =
        fs::read(&paths.data).with_context(|| format!("读取失败: {}", paths.data.display()))?;
    let store: Store = serde_json::from_slice(&bytes).map_err(|e| {
        anyhow!(
            "解析 {} 失败: {e}。原文件与 .bak 已保留，请用 jq 修复后重试。",
            paths.data.display()
        )
    })?;
    if store.version > CURRENT_VERSION {
        bail!(
            "数据文件 version {} 高于本程序支持的 {}，请升级 jj-bookmark。",
            store.version,
            CURRENT_VERSION
        );
    }
    Ok(store)
}

/// 在写锁保护下修改数据：加锁 → 从磁盘重读 → 调用 `f` 修改 → 归一 → 原子写。
/// `f` 的返回值透传给调用方（如新建的 id、受影响条数）。
pub fn mutate<F, T>(paths: &Paths, f: F) -> Result<T>
where
    F: FnOnce(&mut Store) -> Result<T>,
{
    fs::create_dir_all(&paths.dir)
        .with_context(|| format!("创建数据目录失败: {}", paths.dir.display()))?;
    let _guard = FlockGuard::acquire(&paths.lock)?; // 锁在独立锁文件上
    let mut store = read_store(paths)?; // 锁内从磁盘重读，吸收并发写 / 手改
    let result = f(&mut store)?;
    store.normalize(); // 重算所有 *_jst
    write_atomic(paths, &store)?;
    Ok(result)
    // _guard drop → 关闭 fd → 释放 flock
}

/// 原子写：写 tmp → fsync → 备份现有 → rename 覆盖 → fsync 目录。
fn write_atomic(paths: &Paths, store: &Store) -> Result<()> {
    let mut data = serde_json::to_vec_pretty(store).context("序列化 JSON 失败")?;
    data.push(b'\n'); // 末尾换行，文件更友好
    {
        let mut f = File::create(&paths.tmp)
            .with_context(|| format!("创建临时文件失败: {}", paths.tmp.display()))?;
        f.write_all(&data).context("写入临时文件失败")?;
        f.sync_all().context("fsync 临时文件失败")?;
    }
    if paths.data.exists() {
        fs::copy(&paths.data, &paths.bak)
            .with_context(|| format!("备份到 .bak 失败: {}", paths.bak.display()))?;
    }
    fs::rename(&paths.tmp, &paths.data)
        .with_context(|| format!("原子 rename 失败: {}", paths.data.display()))?;
    fsync_dir(&paths.dir)?;
    Ok(())
}

fn fsync_dir(dir: &Path) -> Result<()> {
    let f = File::open(dir).with_context(|| format!("打开目录失败: {}", dir.display()))?;
    f.sync_all().context("fsync 目录失败")?;
    Ok(())
}

/// flock 独占锁的 RAII 句柄：持有到 drop，close fd 时自动释放锁。
struct FlockGuard {
    _file: File,
}

impl FlockGuard {
    fn acquire(lock_path: &Path) -> Result<FlockGuard> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(lock_path)
            .with_context(|| format!("打开锁文件失败: {}", lock_path.display()))?;
        let fd = file.as_raw_fd();
        loop {
            let ret = unsafe { libc::flock(fd, libc::LOCK_EX) };
            if ret == 0 {
                break;
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue; // 被信号打断，重试
            }
            return Err(err).context("flock 加锁失败");
        }
        Ok(FlockGuard { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Bookmark;

    fn temp_paths(tag: &str) -> Paths {
        let mut dir = std::env::temp_dir();
        dir.push(format!("jj-bookmark-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Paths::from_dir(dir)
    }

    #[test]
    fn read_missing_returns_empty() {
        let p = temp_paths("missing");
        let s = read_store(&p).unwrap();
        assert_eq!(s.version, CURRENT_VERSION);
        assert!(s.bookmarks.is_empty());
        let _ = fs::remove_dir_all(&p.dir);
    }

    #[test]
    fn write_then_read_roundtrip() {
        let p = temp_paths("roundtrip");
        mutate(&p, |store| {
            store.bookmarks.push(Bookmark::new(
                1,
                "https://example.com".into(),
                "示例".into(),
                "Tools".into(),
                "".into(),
            ));
            Ok(())
        })
        .unwrap();

        let s = read_store(&p).unwrap();
        assert_eq!(s.bookmarks.len(), 1);
        assert_eq!(s.bookmarks[0].title, "示例");
        // 派生字段已生成
        assert!(!s.bookmarks[0].created_jst.is_empty());
        assert_eq!(s.bookmarks[0].last_visited_jst, "");
        // 文件是 pretty JSON 且中文不转义
        let raw = fs::read_to_string(&p.data).unwrap();
        assert!(raw.contains("  \"title\": \"示例\""));
        let _ = fs::remove_dir_all(&p.dir);
    }

    #[test]
    fn reread_under_lock_absorbs_external_change() {
        // 模拟：mutate 期间数据文件已被「外部」改动，重读应看到它，不覆盖丢失。
        let p = temp_paths("reread");
        mutate(&p, |s| {
            s.bookmarks.push(Bookmark::new(1, "u1".into(), "a".into(), "".into(), "".into()));
            Ok(())
        })
        .unwrap();
        // 直接在磁盘上再加一条（绕过 mutate，模拟另一进程已提交）
        let mut disk = read_store(&p).unwrap();
        disk.bookmarks.push(Bookmark::new(2, "u2".into(), "b".into(), "".into(), "".into()));
        disk.normalize();
        write_atomic(&p, &disk).unwrap();
        // 再 mutate 加第三条：因锁内重读，应基于「磁盘上两条」之上追加
        mutate(&p, |s| {
            assert_eq!(s.bookmarks.len(), 2, "mutate 必须重读到磁盘最新状态");
            s.bookmarks.push(Bookmark::new(3, "u3".into(), "c".into(), "".into(), "".into()));
            Ok(())
        })
        .unwrap();
        let s = read_store(&p).unwrap();
        assert_eq!(s.bookmarks.len(), 3);
        let _ = fs::remove_dir_all(&p.dir);
    }
}
