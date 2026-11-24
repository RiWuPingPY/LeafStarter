//! 本机的文件仓库和存档历史（ROADMAP 5.7「历史」）。
//!
//! - 对象：按 SHA-1 存一份（`<数据目录>/sync/objects/ab/abcdef…`），几份历史里没变的区块文件只占一份空间
//! - 快照：一个世界某一时刻的文件清单（相对路径 → SHA-1、大小），存成 JSON
//! - 哈希缓存：大小和修改时间都没变的文件不重新算哈希（世界动辄几百 MB）

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::base::util::now_ms;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub sha1: String,
    pub size: u64,
}

/// 一个文件夹的清单：相对路径（用 `/`）→ 内容
pub type Files = BTreeMap<String, Entry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub at: u64,
    /// 哪台设备上的（本机、手机、另一台电脑）
    pub device: String,
    /// 为什么存：manual / before-restore / before-sync / auto
    pub reason: String,
    pub files: Files,
}

impl Snapshot {
    pub fn size(&self) -> u64 {
        self.files.values().map(|e| e.size).sum()
    }
}

pub struct Store {
    root: PathBuf,
    /// 绝对路径 → (大小, 修改时间, sha1)
    cache: Mutex<HashMap<String, (u64, u64, String)>>,
}

/// 不进快照的文件：游戏开着时的锁、我们自己的临时文件
fn skip(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name == "session.lock" || name.ends_with(".ls-tmp") || name.ends_with(".tmp")
}

fn mtime(m: &std::fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn sha1_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(p).with_context(|| format!("读不了 {}", p.display()))?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

impl Store {
    pub fn open(data_dir: &Path) -> Store {
        let root = data_dir.join("sync");
        let cache = std::fs::read(root.join("hashcache.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Store { root, cache: Mutex::new(cache) }
    }

    fn save_cache(&self) {
        let c = self.cache.lock().unwrap();
        let _ = std::fs::create_dir_all(&self.root);
        let _ = std::fs::write(self.root.join("hashcache.json"), serde_json::to_vec(&*c).unwrap_or_default());
    }

    pub fn object_path(&self, sha1: &str) -> PathBuf {
        self.root.join("objects").join(&sha1[..2.min(sha1.len())]).join(sha1)
    }

    pub fn has(&self, sha1: &str) -> bool {
        self.object_path(sha1).is_file()
    }

    /// 文件的 SHA-1（大小、修改时间没变就用缓存）
    pub fn hash(&self, p: &Path) -> Result<Entry> {
        let m = std::fs::metadata(p)?;
        let key = p.to_string_lossy().into_owned();
        let (size, mt) = (m.len(), mtime(&m));
        if let Some((s, t, h)) = self.cache.lock().unwrap().get(&key) {
            if *s == size && *t == mt {
                return Ok(Entry { sha1: h.clone(), size });
            }
        }
        let h = sha1_file(p)?;
        self.cache.lock().unwrap().insert(key, (size, mt, h.clone()));
        Ok(Entry { sha1: h, size })
    }

    /// 列出一个文件夹里的文件和哈希（相对路径用 `/`）
    pub fn scan(&self, dir: &Path) -> Result<Files> {
        let mut out = Files::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).with_context(|| format!("读不了 {}", d.display()))?.flatten() {
                let p = e.path();
                let ft = e.file_type()?;
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() {
                    let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                    if !skip(&rel) {
                        out.insert(rel, self.hash(&p)?);
                    }
                }
            }
        }
        self.save_cache();
        Ok(out)
    }

    /// 把一个文件收进仓库（已经有了就不动）
    pub fn put_file(&self, p: &Path, e: &Entry) -> Result<()> {
        let dst = self.object_path(&e.sha1);
        if dst.is_file() {
            return Ok(());
        }
        std::fs::create_dir_all(dst.parent().unwrap())?;
        let tmp = dst.with_extension("ls-tmp");
        std::fs::copy(p, &tmp).with_context(|| format!("复制 {} 到 {} 失败", p.display(), tmp.display()))?;
        // 复制的时候游戏可能正好在写：对一下哈希，对不上就算了（下次再存）
        if sha1_file(&tmp)? != e.sha1 {
            let _ = std::fs::remove_file(&tmp);
            bail!("{} 正在被改写，先关掉游戏再存", p.display());
        }
        crate::base::util::move_file(&tmp, &dst).with_context(|| format!("{} 换不成 {}", tmp.display(), dst.display()))?;
        Ok(())
    }

    /// 收一段数据（从别的设备传来的）；哈希对不上拒收
    pub fn put_bytes(&self, sha1: &str, data: &[u8]) -> Result<()> {
        let got: String = Sha1::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        if got != sha1 {
            bail!("收到的文件校验不对");
        }
        let dst = self.object_path(sha1);
        if dst.is_file() {
            return Ok(());
        }
        std::fs::create_dir_all(dst.parent().unwrap())?;
        let tmp = dst.with_extension("ls-tmp");
        std::fs::write(&tmp, data).with_context(|| format!("写不了 {}", tmp.display()))?;
        crate::base::util::move_file(&tmp, &dst).with_context(|| format!("{} 换不成 {}", tmp.display(), dst.display()))?;
        Ok(())
    }

    // ---------- 存档历史 ----------

    fn history_dir(&self, key: &str) -> PathBuf {
        self.root.join("history").join(key)
    }

    /// 给一个文件夹拍快照：文件收进仓库，清单存起来；只保留最近 `keep` 份
    pub fn snapshot(&self, key: &str, dir: &Path, device: &str, reason: &str, keep: usize) -> Result<Snapshot> {
        let files = self.scan(dir)?;
        for (rel, e) in &files {
            self.put_file(&dir.join(rel), e)?;
        }
        // 和最近一份一模一样就不重复存
        if let Some(last) = self.list(key).into_iter().next() {
            if last.files == files {
                return Ok(last);
            }
        }
        let at = now_ms();
        let snap = Snapshot { id: format!("{at}"), at, device: device.into(), reason: reason.into(), files };
        let hd = self.history_dir(key);
        std::fs::create_dir_all(&hd)?;
        std::fs::write(hd.join(format!("{}.json", snap.id)), serde_json::to_vec(&snap)?)?;
        let all = self.list(key);
        for old in all.iter().skip(keep.max(1)) {
            let _ = std::fs::remove_file(hd.join(format!("{}.json", old.id)));
        }
        if all.len() > keep {
            self.gc();
        }
        Ok(snap)
    }

    /// 一个世界的历史，新的在前
    pub fn list(&self, key: &str) -> Vec<Snapshot> {
        let mut v: Vec<Snapshot> = std::fs::read_dir(self.history_dir(key))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        v.sort_by(|a, b| b.at.cmp(&a.at));
        v
    }

    /// 退回到某一份：先给现在的样子拍一份（后悔了还能回来），再按清单写回去，多出来的文件删掉
    pub fn restore(&self, key: &str, dir: &Path, id: &str, device: &str, keep: usize) -> Result<()> {
        let snap = self.list(key).into_iter().find(|s| s.id == id).context("找不到这份历史")?;
        for e in snap.files.values() {
            if !self.has(&e.sha1) {
                bail!("这份历史缺文件，退回不了");
            }
        }
        if dir.is_dir() {
            self.snapshot(key, dir, device, "before-restore", keep + 1)?;
        }
        self.write_files(dir, &snap.files)
    }

    /// 让文件夹变成 `files` 的样子：缺的、变了的从仓库拿，多的删掉
    pub fn write_files(&self, dir: &Path, files: &Files) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let now = self.scan(dir)?;
        for (rel, e) in files {
            if now.get(rel) == Some(e) {
                continue;
            }
            let dst = safe_join(dir, rel)?;
            std::fs::create_dir_all(dst.parent().unwrap())?;
            let tmp = dst.with_extension("ls-tmp");
            std::fs::copy(self.object_path(&e.sha1), &tmp).with_context(|| format!("写 {} 失败", rel))?;
            crate::base::util::move_file(&tmp, &dst).with_context(|| format!("换不上 {}（游戏是不是开着这个世界？）", rel))?;
        }
        for rel in now.keys().filter(|r| !files.contains_key(*r)) {
            let _ = std::fs::remove_file(safe_join(dir, rel)?);
        }
        Ok(())
    }

    /// 删掉没有任何历史再引用的对象
    pub fn gc(&self) {
        let mut used = HashSet::new();
        for k in std::fs::read_dir(self.root.join("history")).into_iter().flatten().flatten() {
            for s in self.list(&k.file_name().to_string_lossy()) {
                used.extend(s.files.into_values().map(|e| e.sha1));
            }
        }
        for sub in std::fs::read_dir(self.root.join("objects")).into_iter().flatten().flatten() {
            for o in std::fs::read_dir(sub.path()).into_iter().flatten().flatten() {
                if !used.contains(&*o.file_name().to_string_lossy()) {
                    let _ = std::fs::remove_file(o.path());
                }
            }
        }
    }
}

/// 相对路径拼到目录下，不许跑出去（`..`、盘符、开头的 `/`）
pub fn safe_join(dir: &Path, rel: &str) -> Result<PathBuf> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') || rel.contains(':') || rel.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
        bail!("文件路径不对：{rel}");
    }
    Ok(dir.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_restore_and_dedupe() {
        let base = std::env::temp_dir().join(format!("ls-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (data, world) = (base.join("data"), base.join("world"));
        std::fs::create_dir_all(world.join("region")).unwrap();
        std::fs::write(world.join("level.dat"), b"v1").unwrap();
        std::fs::write(world.join("region/r.0.0.mca"), vec![7u8; 100_000]).unwrap();
        std::fs::write(world.join("session.lock"), b"x").unwrap();
        let st = Store::open(&data);
        let s1 = st.snapshot("w", &world, "本机", "manual", 3).unwrap();
        assert_eq!(s1.files.len(), 2, "session.lock 不进快照");
        // 没变：不重复存
        assert_eq!(st.snapshot("w", &world, "本机", "manual", 3).unwrap().id, s1.id);
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(world.join("level.dat"), b"v2").unwrap();
        std::fs::write(world.join("region/r.1.0.mca"), b"new").unwrap();
        let s2 = st.snapshot("w", &world, "本机", "manual", 3).unwrap();
        assert_ne!(s1.id, s2.id);
        // 区块文件没变，仓库里只有一份
        let objects = walk(&data.join("sync/objects"));
        assert_eq!(objects, 4, "v1、v2、r.0.0、r.1.0");
        // 退回第一份：多出来的 r.1.0 删掉，level.dat 回到 v1
        st.restore("w", &world, &s1.id, "本机", 3).unwrap();
        assert_eq!(std::fs::read(world.join("level.dat")).unwrap(), b"v1");
        assert!(!world.join("region/r.1.0.mca").exists());
        // 退回前的样子（和 s2 一样）还在历史里，后悔了能回来
        assert!(st.list("w").iter().any(|s| s.id == s2.id));
        assert!(safe_join(&world, "../x").is_err());
        assert!(safe_join(&world, "a/../../x").is_err());
        assert!(safe_join(&world, "C:/x").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    fn walk(d: &Path) -> usize {
        std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| if e.path().is_dir() { walk(&e.path()) } else { 1 }).sum()
    }

    #[test]
    fn keeps_only_recent() {
        let base = std::env::temp_dir().join(format!("ls-store2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let w = base.join("w");
        std::fs::create_dir_all(&w).unwrap();
        let st = Store::open(&base.join("data"));
        for k in 0..5 {
            std::fs::write(w.join("level.dat"), format!("v{k}")).unwrap();
            st.snapshot("k", &w, "本机", "auto", 3).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert_eq!(st.list("k").len(), 3);
        assert_eq!(walk(&base.join("data/sync/objects")), 3, "旧的对象被清掉");
        let _ = std::fs::remove_dir_all(&base);
    }
}
