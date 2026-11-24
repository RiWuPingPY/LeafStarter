//! 把 Modrinth 上的东西装进实例：自动带上必需的前置，已经装了的前置不重复装，
//! 同一个项目的旧版本装好新版本后删掉。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use super::modrinth::{self, VFile, Version};
use crate::base::net::{FileJob, Net, Progress};

/// 资源放在实例的哪个文件夹
pub fn folder_of(kind: &str) -> &'static str {
    match kind {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    }
}

/// 文件夹里每个文件的 SHA-1 → 文件名（.disabled 的也算）
pub fn local_hashes(dir: &Path) -> HashMap<String, String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| Some((crate::sync::store::sha1_file(&e.path()).ok()?, e.file_name().to_string_lossy().into_owned())))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub project: String,
    pub title: String,
    pub version: String,
    pub file: VFile,
    /// 自己点的还是自动带上的前置
    pub dep: bool,
    /// 换掉本地的哪个旧文件
    pub replaces: Option<String>,
}

/// 算出要装哪些：选中的那个 + 递归的必需前置（本地已有的跳过）
pub async fn plan(http: &reqwest::Client, dir: &Path, project: &str, version: Option<&str>, mc: Option<&str>, loader: Option<&str>) -> Result<Vec<Item>> {
    let local = local_hashes(dir);
    let known = modrinth::by_hashes(http, &local.keys().cloned().collect::<Vec<_>>()).await.unwrap_or_default();
    // 项目 id → 本地文件名
    let installed: HashMap<String, String> = known.iter().filter_map(|(h, v)| Some((v.project_id.clone(), local.get(h)?.clone()))).collect();
    let installed_versions: HashSet<String> = known.values().map(|v| v.id.clone()).collect();

    let first = match version {
        Some(v) => modrinth::version(http, v).await?,
        None => {
            let list = modrinth::versions(http, project, mc, loader).await?;
            match modrinth::pick(&list) {
                Some(v) => v.clone(),
                None => bail!("这个项目没有适合 {}{} 的版本", mc.unwrap_or(""), loader.map(|l| format!(" {l}")).unwrap_or_default()),
            }
        }
    };
    if installed_versions.contains(&first.id) {
        bail!("已经装了这个版本");
    }
    let mut out: Vec<Item> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(Version, bool)> = VecDeque::from([(first, false)]);
    while let Some((v, dep)) = queue.pop_front() {
        if !seen.insert(v.project_id.clone()) {
            continue;
        }
        let Some(file) = v.primary().cloned() else { continue };
        for (pid, vid, kind) in &v.deps {
            if kind != "required" {
                continue;
            }
            let Some(pid) = pid.clone() else {
                if let Some(vid) = vid {
                    let dv = modrinth::version(http, vid).await?;
                    if !installed.contains_key(&dv.project_id) {
                        queue.push_back((dv, true));
                    }
                }
                continue;
            };
            if installed.contains_key(&pid) || seen.contains(&pid) {
                continue;
            }
            let dv = match vid {
                Some(vid) => modrinth::version(http, vid).await?,
                None => {
                    let list = modrinth::versions(http, &pid, mc, loader).await?;
                    match modrinth::pick(&list) {
                        Some(x) => x.clone(),
                        None => bail!("前置 {pid} 没有适合这个实例的版本，装不了"),
                    }
                }
            };
            queue.push_back((dv, true));
        }
        out.push(Item { replaces: installed.get(&v.project_id).cloned(), project: v.project_id.clone(), title: String::new(), version: v.number.clone(), file, dep });
    }
    // 补上名字
    let ids: Vec<String> = out.iter().map(|i| i.project.clone()).collect();
    if let Ok(names) = modrinth::projects(http, &ids).await {
        for i in &mut out {
            if let Some(p) = names.get(&i.project) {
                i.title = p["title"].as_str().unwrap_or("").to_string();
            }
        }
    }
    Ok(out)
}

/// 照着计划下载，下好了再删旧版本；返回装好的文件名
pub async fn apply(net: &Arc<Net>, dir: &Path, items: &[Item], prog: &Arc<Progress>) -> Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let mut jobs = vec![];
    for i in items {
        let name = safe_name(&i.file.filename)?;
        jobs.push(FileJob { url: i.file.url.clone(), path: dir.join(&name), sha1: Some(i.file.sha1.clone()).filter(|s| !s.is_empty()), size: Some(i.file.size) });
    }
    crate::game::install::download_all(net, &jobs, prog).await?;
    for i in items {
        if let Some(old) = &i.replaces {
            if old != &i.file.filename {
                // 旧版本放进回收站：更新坏了还能拿回来
                let p = dir.join(old);
                if crate::instance::to_recycle_bin(&p).is_err() {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
    }
    Ok(items.iter().map(|i| i.file.filename.clone()).collect())
}

/// 下载来的文件名只能是一层，不能带路径
pub fn safe_name(n: &str) -> Result<String> {
    let n = n.trim();
    if n.is_empty() || n.contains(['/', '\\', ':']) || n == "." || n == ".." {
        bail!("文件名不对：{n}");
    }
    Ok(n.to_string())
}

pub fn dir_of(game_dir: &Path, kind: &str) -> PathBuf {
    game_dir.join(folder_of(kind))
}
