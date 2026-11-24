//! 拉和推：把对方的实例（Mod、配置）和世界拿过来，玩完把世界传回去。
//!
//! 每个世界记一个「上次同步时的样子」（清单哈希 = base）：
//! - 只有一边变了 → 照着变了的那边改
//! - 两边都变了 → 冲突，问用户要哪边（另一边先存进历史，不会丢）

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::lan::{files_hash, instance_files};
use super::peer::Client;
use super::store::{safe_join, Files, Store};

/// 本机实例和对方实例的对应关系，存在 `<数据目录>/sync/links.json`，键是本机游戏目录
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub peer: String,
    pub remote: String,
    /// 世界 → 上次同步时的清单哈希
    #[serde(default)]
    pub worlds: BTreeMap<String, String>,
}

pub fn load_links(data_dir: &Path) -> BTreeMap<String, Link> {
    std::fs::read(data_dir.join("sync/links.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_links(data_dir: &Path, l: &BTreeMap<String, Link>) -> Result<()> {
    std::fs::create_dir_all(data_dir.join("sync"))?;
    std::fs::write(data_dir.join("sync/links.json"), serde_json::to_vec_pretty(l)?)?;
    Ok(())
}

pub fn link_key(gd: &Path) -> String {
    gd.to_string_lossy().to_lowercase()
}

/// 把 `want` 里缺的对象拿过来：本机文件夹里有一样的就不下载
async fn fetch(c: &Client, store: &Store, want: &Files, local: &HashMap<String, PathBuf>, prog: &(dyn Fn(String) + Sync)) -> Result<()> {
    let need: Vec<_> = want.values().filter(|e| !store.has(&e.sha1) && !local.contains_key(&e.sha1)).collect();
    let total: u64 = need.iter().map(|e| e.size).sum();
    let mut done = 0u64;
    for (k, e) in need.iter().enumerate() {
        prog(format!("传文件 {}/{}（{}）", k + 1, need.len(), crate::base::util::human_size(total.saturating_sub(done))));
        let (_, data) = c.call("/get", json!({ "sha1": e.sha1 }), &[]).await?;
        store.put_bytes(&e.sha1, &data)?;
        done += e.size;
    }
    Ok(())
}

/// 让 `dir` 变成 `want` 的样子。`mirror` 里的前缀（比如 `mods/`）多出来的文件删掉，别的地方只加不删
fn apply(store: &Store, dir: &Path, want: &Files, now: &Files, local: &HashMap<String, PathBuf>, mirror: &[&str]) -> Result<()> {
    for (rel, e) in want {
        if now.get(rel) == Some(e) {
            continue;
        }
        let dst = safe_join(dir, rel)?;
        std::fs::create_dir_all(dst.parent().unwrap())?;
        let src = if store.has(&e.sha1) { store.object_path(&e.sha1) } else { local.get(&e.sha1).cloned().context("缺文件")? };
        let tmp = dst.with_extension("ls-tmp");
        std::fs::copy(&src, &tmp).with_context(|| format!("写 {rel} 失败（{} → {}）", src.display(), tmp.display()))?;
        crate::base::util::move_file(&tmp, &dst).with_context(|| format!("换不上 {rel}（游戏是不是开着？）"))?;
    }
    for rel in now.keys() {
        if !want.contains_key(rel) && (mirror.contains(&"") || mirror.iter().any(|m| !m.is_empty() && rel.starts_with(m))) {
            let _ = std::fs::remove_file(safe_join(dir, rel)?);
        }
    }
    Ok(())
}

fn by_hash(dir: &Path, files: &Files) -> HashMap<String, PathBuf> {
    files.iter().map(|(rel, e)| (e.sha1.clone(), dir.join(rel))).collect()
}

/// 拉实例：Mod 和 mods 文件夹完全一致（多的删掉），配置、资源包只加不删
pub async fn pull_instance(c: &Client, store: &Store, remote: &str, gd: &Path, prog: &(dyn Fn(String) + Sync)) -> Result<usize> {
    prog("看对方的 Mod 和配置".into());
    let (v, _) = c.call("/files", json!({ "key": remote }), &[]).await?;
    let want: Files = serde_json::from_value(v["files"].clone())?;
    std::fs::create_dir_all(gd)?;
    let now = instance_files(store, gd)?;
    let local = by_hash(gd, &now);
    fetch(c, store, &want, &local, prog).await?;
    let changed = want.iter().filter(|(r, e)| now.get(*r) != Some(e)).count() + now.keys().filter(|r| r.starts_with("mods/") && !want.contains_key(*r)).count();
    apply(store, gd, &want, &now, &local, &["mods/"])?;
    Ok(changed)
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Outcome {
    /// 两边一样
    Same,
    /// 改好了
    Done { files: usize },
    /// 两边都改过：要用户选
    Conflict,
}

/// 拉一个世界。`base` 是上次同步时的清单哈希；本机在那之后改过、对方也改过就是冲突（`force` 用对方的）
pub async fn pull_world(c: &Client, store: &Store, remote: &str, world: &str, gd: &Path, base: Option<&str>, force: bool, hist_key: &str, prog: &(dyn Fn(String) + Sync)) -> Result<(Outcome, String)> {
    crate::instance::check_name(world)?;
    let (v, _) = c.call("/files", json!({ "key": remote, "world": world }), &[]).await?;
    let want: Files = serde_json::from_value(v["files"].clone())?;
    let theirs = files_hash(&want);
    let dir = gd.join("saves").join(world);
    let now = if dir.is_dir() { store.scan(&dir)? } else { Files::new() };
    let mine = files_hash(&now);
    if mine == theirs {
        return Ok((Outcome::Same, theirs));
    }
    if !now.is_empty() && base.is_some_and(|b| b != mine) && !force {
        return Ok((Outcome::Conflict, mine));
    }
    if !now.is_empty() {
        store.snapshot(hist_key, &dir, "这台电脑", "before-sync", crate::commands::sync::KEEP)?;
    }
    let local = by_hash(&dir, &now);
    fetch(c, store, &want, &local, prog).await?;
    let n = want.iter().filter(|(r, e)| now.get(*r) != Some(e)).count();
    std::fs::create_dir_all(&dir)?;
    apply(store, &dir, &want, &now, &local, &[""])?;
    Ok((Outcome::Done { files: n }, theirs))
}

/// 推一个世界回去。对方在 base 之后也改过就是冲突（`force` 用这边的，对方那份会先存进它的历史）
pub async fn push_world(c: &Client, store: &Store, remote: &str, world: &str, gd: &Path, base: Option<&str>, force: bool, prog: &(dyn Fn(String) + Sync)) -> Result<(Outcome, String)> {
    crate::instance::check_name(world)?;
    let dir = gd.join("saves").join(world);
    let files = store.scan(&dir)?;
    let mine = files_hash(&files);
    if base == Some(mine.as_str()) && !force {
        return Ok((Outcome::Same, mine));
    }
    let sha1s: Vec<&String> = files.values().map(|e| &e.sha1).collect();
    let (v, _) = c.call("/missing", json!({ "sha1s": sha1s }), &[]).await?;
    let missing: Vec<String> = serde_json::from_value(v["missing"].clone())?;
    let local = by_hash(&dir, &files);
    for (k, s) in missing.iter().enumerate() {
        prog(format!("传文件 {}/{}", k + 1, missing.len()));
        let p = local.get(s).context("文件在传的时候变了，关掉游戏再试")?;
        c.call("/put", json!({ "sha1": s }), &std::fs::read(p)?).await?;
    }
    let (v, _) = c.call("/apply", json!({ "key": remote, "world": world, "files": files, "base": base.unwrap_or(""), "force": force }), &[]).await?;
    if v["conflict"].as_bool().unwrap_or(false) {
        return Ok((Outcome::Conflict, mine));
    }
    Ok((Outcome::Done { files: missing.len() }, mine))
}
