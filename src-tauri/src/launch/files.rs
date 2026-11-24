//! 启动前检查文件：客户端 jar、库、本地库、日志配置、资源索引和资源对象，缺的补上。
//!
//! 每次启动都要做，所以只比文件在不在、大小对不对，不算哈希（几千个资源文件算一遍要很久）；
//! 真要下载的文件，下载完照常校验 SHA1。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::base::net::{FileJob, Net, Progress};
use crate::base::util::{join_rel, rules_allow, Coord, OsTarget};
use crate::game::{install, mojang};
use crate::launch::resolve::Resolved;

/// 这个版本要的全部文件（资源索引要先在本地，才知道有哪些资源对象）
pub async fn required(net: &Arc<Net>, root: &Path, r: &Resolved, prog: &Arc<Progress>) -> Result<Vec<FileJob>> {
    let os = OsTarget::current();
    let mut jobs = mojang::base_jobs(&r.json, root, &r.jar_id, os);
    if let Some(idx) = mojang::asset_index_job(&r.json, root) {
        if !quick_ok(&idx).await {
            install::download_all(net, std::slice::from_ref(&idx), prog).await.context("资源索引下载失败")?;
        }
        let index: Value = serde_json::from_slice(&std::fs::read(&idx.path)?).context("资源索引格式不对")?;
        jobs.extend(mojang::asset_object_jobs(&index, root));
        jobs.push(idx);
    }
    Ok(jobs)
}

/// 安装器在本地生成、没有下载地址的库（Forge 的处理器产物）：缺了只能重新安装
pub fn missing_generated(json: &Value, root: &Path) -> Vec<PathBuf> {
    let os = OsTarget::current();
    let libs = root.join("libraries");
    json.get("libraries")
        .and_then(|l| l.as_array())
        .into_iter()
        .flatten()
        .filter(|lib| rules_allow(lib.get("rules"), os))
        .filter_map(|lib| {
            let art = lib.get("downloads")?.get("artifact")?;
            if !art.get("url").and_then(|u| u.as_str()).unwrap_or("").is_empty() {
                return None;
            }
            let rel = art.get("path").and_then(|p| p.as_str()).map(String::from).or_else(|| Coord::parse(lib.get("name")?.as_str()?).map(|c| c.path()))?;
            let p = join_rel(&libs, &rel);
            (!p.is_file()).then_some(p)
        })
        .collect()
}

async fn quick_ok(job: &FileJob) -> bool {
    match tokio::fs::metadata(&job.path).await {
        Ok(m) => m.is_file() && m.len() > 0 && job.size.map_or(true, |s| s == m.len()),
        Err(_) => false,
    }
}

/// 挑出缺的或大小不对的
pub async fn missing(jobs: Vec<FileJob>) -> Vec<FileJob> {
    let mut out = vec![];
    for j in jobs {
        if !quick_ok(&j).await {
            out.push(j);
        }
    }
    out
}

/// 很老的版本（1.7.2 及更早）的资源要按文件名摆出来：virtual 摆进 assets/virtual/legacy，
/// map_to_resources 摆进游戏目录的 resources。返回 `${game_assets}` 该指向的目录。
pub fn legacy_assets(json: &Value, root: &Path, game_dir: &Path) -> Result<PathBuf> {
    let assets = root.join("assets");
    let Some(id) = json.get("assetIndex").and_then(|i| i.get("id")).and_then(|i| i.as_str()) else { return Ok(assets) };
    let Ok(text) = std::fs::read(assets.join("indexes").join(format!("{}.json", id))) else { return Ok(assets) };
    let index: Value = serde_json::from_slice(&text)?;
    let to_resources = index.get("map_to_resources").and_then(|v| v.as_bool()) == Some(true);
    let virtual_ = index.get("virtual").and_then(|v| v.as_bool()) == Some(true);
    if !to_resources && !virtual_ {
        return Ok(assets);
    }
    let dest = if to_resources { game_dir.join("resources") } else { assets.join("virtual").join("legacy") };
    for (name, obj) in index.get("objects").and_then(|o| o.as_object()).into_iter().flatten() {
        let Some(hash) = obj.get("hash").and_then(|h| h.as_str()).filter(|h| h.len() > 2) else { continue };
        let to = join_rel(&dest, name);
        if to.is_file() {
            continue;
        }
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        let from = assets.join("objects").join(&hash[..2]).join(hash);
        if from.is_file() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(dest)
}

pub fn check_generated(json: &Value, root: &Path) -> Result<()> {
    let gen = missing_generated(json, root);
    if let Some(first) = gen.first() {
        bail!("这个版本缺 {} 个安装器生成的文件（比如 {}），下载补不回来。重新安装一次这个版本就好", gen.len(), first.display());
    }
    Ok(())
}
