//! 原版：版本清单、版本 JSON 里的文件清单、与加载器 JSON 合并。

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::net::{FileJob, Net};
use crate::util::{join_rel, rules_allow, Coord, OsTarget};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
const ASSETS_URL: &str = "https://resources.download.minecraft.net/";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

pub async fn fetch_manifest(net: &Net) -> Result<Manifest> {
    net.get_json(MANIFEST_URL).await
}

/// 取原版版本 JSON
pub async fn fetch_version_json(net: &Net, v: &ManifestVersion) -> Result<Value> {
    let bytes = net.get_bytes(&v.url).await?;
    serde_json::from_slice(&bytes).with_context(|| format!("版本 {} 的 JSON 格式不对", v.id))
}

/// 一个库条目对应要下载的文件（主 jar + 当前平台的 natives）
pub fn library_jobs(lib: &Value, libs_dir: &Path, os: OsTarget) -> Vec<FileJob> {
    let mut out = vec![];
    if !rules_allow(lib.get("rules"), os) {
        return out;
    }
    // 老 Forge 的 versionInfo 里标记了客户端不需要的库
    if lib.get("clientreq").and_then(|v| v.as_bool()) == Some(false) {
        return out;
    }
    let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let downloads = lib.get("downloads");

    if let Some(art) = downloads.and_then(|d| d.get("artifact")) {
        if let Some(job) = artifact_job(art, name, libs_dir) {
            out.push(job);
        }
    } else if downloads.is_none() && !name.is_empty() {
        // 只有 name + url 的写法（Fabric / Quilt / 老 Forge）
        if let Some(c) = Coord::parse(name) {
            let base = lib.get("url").and_then(|u| u.as_str()).unwrap_or(LIBRARIES_URL);
            let base = if base.ends_with('/') { base.to_string() } else { format!("{}/", base) };
            let rel = c.path();
            out.push(FileJob {
                url: format!("{}{}", base, rel),
                path: join_rel(libs_dir, &rel),
                sha1: lib.get("sha1").and_then(|s| s.as_str()).map(String::from),
                size: lib.get("size").and_then(|s| s.as_u64()),
            });
        }
    }

    // 1.18 及更早的 natives 分类器写法
    if let Some(classifier) = lib.get("natives").and_then(|n| n.get(os.name())).and_then(|c| c.as_str()) {
        let classifier = classifier.replace("${arch}", "64");
        if let Some(art) = downloads.and_then(|d| d.get("classifiers")).and_then(|c| c.get(&classifier)) {
            if let Some(job) = artifact_job(art, name, libs_dir) {
                out.push(job);
            }
        } else if let Some(mut c) = Coord::parse(name) {
            c.classifier = Some(classifier);
            let rel = c.path();
            out.push(FileJob { url: format!("{}{}", LIBRARIES_URL, rel), path: join_rel(libs_dir, &rel), sha1: None, size: None });
        }
    }
    out
}

fn artifact_job(art: &Value, name: &str, libs_dir: &Path) -> Option<FileJob> {
    let url = art.get("url").and_then(|u| u.as_str()).unwrap_or("");
    // url 为空的是安装器处理器生成的文件，不需要下载
    if url.is_empty() {
        return None;
    }
    let rel = art
        .get("path")
        .and_then(|p| p.as_str())
        .map(String::from)
        .or_else(|| Coord::parse(name).map(|c| c.path()))?;
    Some(FileJob {
        url: url.to_string(),
        path: join_rel(libs_dir, &rel),
        sha1: art.get("sha1").and_then(|s| s.as_str()).map(String::from),
        size: art.get("size").and_then(|s| s.as_u64()),
    })
}

/// 版本 JSON 里所有库在本地的路径（用于把文件同步到启动器目录）
pub fn library_paths(version: &Value, os: OsTarget) -> Vec<String> {
    let mut out = vec![];
    for lib in version.get("libraries").and_then(|l| l.as_array()).into_iter().flatten() {
        if !rules_allow(lib.get("rules"), os) {
            continue;
        }
        let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let art_path = lib
            .get("downloads")
            .and_then(|d| d.get("artifact"))
            .and_then(|a| a.get("path"))
            .and_then(|p| p.as_str())
            .map(String::from);
        if let Some(p) = art_path.or_else(|| Coord::parse(name).map(|c| c.path())) {
            out.push(p);
        }
        if let Some(classifier) = lib.get("natives").and_then(|n| n.get(os.name())).and_then(|c| c.as_str()) {
            let classifier = classifier.replace("${arch}", "64");
            let p = lib
                .get("downloads")
                .and_then(|d| d.get("classifiers"))
                .and_then(|c| c.get(&classifier))
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
                .map(String::from)
                .or_else(|| {
                    Coord::parse(name).map(|mut c| {
                        c.classifier = Some(classifier.clone());
                        c.path()
                    })
                });
            if let Some(p) = p {
                out.push(p);
            }
        }
    }
    out
}

/// 原版需要的全部文件（不含资源对象，资源要先拿到索引才知道）
pub fn base_jobs(version: &Value, root: &Path, jar_name: &str, os: OsTarget) -> Vec<FileJob> {
    let mut jobs = vec![];
    if let Some(client) = version.get("downloads").and_then(|d| d.get("client")) {
        if let Some(url) = client.get("url").and_then(|u| u.as_str()) {
            jobs.push(FileJob {
                url: url.to_string(),
                path: root.join("versions").join(jar_name).join(format!("{}.jar", jar_name)),
                sha1: client.get("sha1").and_then(|s| s.as_str()).map(String::from),
                size: client.get("size").and_then(|s| s.as_u64()),
            });
        }
    }
    let libs_dir = root.join("libraries");
    for lib in version.get("libraries").and_then(|l| l.as_array()).into_iter().flatten() {
        jobs.extend(library_jobs(lib, &libs_dir, os));
    }
    if let Some(log) = version.get("logging").and_then(|l| l.get("client")).and_then(|c| c.get("file")) {
        if let (Some(id), Some(url)) = (log.get("id").and_then(|i| i.as_str()), log.get("url").and_then(|u| u.as_str())) {
            jobs.push(FileJob {
                url: url.to_string(),
                path: root.join("assets").join("log_configs").join(id),
                sha1: log.get("sha1").and_then(|s| s.as_str()).map(String::from),
                size: log.get("size").and_then(|s| s.as_u64()),
            });
        }
    }
    dedupe(jobs)
}

pub fn asset_index_job(version: &Value, root: &Path) -> Option<FileJob> {
    let idx = version.get("assetIndex")?;
    let id = idx.get("id")?.as_str()?;
    Some(FileJob {
        url: idx.get("url")?.as_str()?.to_string(),
        path: root.join("assets").join("indexes").join(format!("{}.json", id)),
        sha1: idx.get("sha1").and_then(|s| s.as_str()).map(String::from),
        size: idx.get("size").and_then(|s| s.as_u64()),
    })
}

/// 资源索引 → 每个资源对象的下载任务
pub fn asset_object_jobs(index: &Value, root: &Path) -> Vec<FileJob> {
    let objects_dir = root.join("assets").join("objects");
    let mut seen = HashSet::new();
    let mut jobs = vec![];
    for obj in index.get("objects").and_then(|o| o.as_object()).into_iter().flat_map(|m| m.values()) {
        let Some(hash) = obj.get("hash").and_then(|h| h.as_str()) else { continue };
        if hash.len() < 2 || !seen.insert(hash.to_string()) {
            continue;
        }
        let prefix = &hash[..2];
        jobs.push(FileJob {
            url: format!("{}{}/{}", ASSETS_URL, prefix, hash),
            path: objects_dir.join(prefix).join(hash),
            sha1: Some(hash.to_string()),
            size: obj.get("size").and_then(|s| s.as_u64()),
        });
    }
    jobs
}

pub fn asset_object_paths(index: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    index
        .get("objects")
        .and_then(|o| o.as_object())
        .into_iter()
        .flat_map(|m| m.values())
        .filter_map(|obj| obj.get("hash").and_then(|h| h.as_str()))
        .filter(|h| h.len() >= 2 && seen.insert(h.to_string()))
        .map(|h| format!("assets/objects/{}/{}", &h[..2], h))
        .collect()
}

pub fn dedupe(jobs: Vec<FileJob>) -> Vec<FileJob> {
    let mut seen = HashSet::new();
    jobs.into_iter().filter(|j| seen.insert(j.path.clone())).collect()
}

fn lib_key(lib: &Value) -> Option<String> {
    let name = lib.get("name")?.as_str()?;
    let c = Coord::parse(name)?;
    Some(c.dedupe_key())
}

/// 把加载器的 JSON 合并进原版，得到一个独立版本（和 PCL 的做法一致，不依赖 inheritsFrom）
pub fn merge(vanilla: &Value, loader: Option<&Value>, new_id: &str, mc: &str) -> Value {
    let mut out = vanilla.clone();
    let obj = out.as_object_mut().expect("version json is object");
    obj.insert("id".into(), json!(new_id));
    obj.remove("inheritsFrom");
    obj.remove("jar");
    obj.remove("_comment_");
    obj.insert("clientVersion".into(), json!(mc));

    if let Some(loader) = loader {
        if let Some(main) = loader.get("mainClass") {
            obj.insert("mainClass".into(), main.clone());
        }
        // 库：加载器的排前面，原版里与加载器同名的库以加载器的版本为准。
        // 原版自己内部不去重——老版本会用 rules 区分同一个库在不同系统上的两个版本。
        let loader_libs = loader.get("libraries").and_then(|l| l.as_array()).cloned().unwrap_or_default();
        let vanilla_libs = vanilla.get("libraries").and_then(|l| l.as_array()).cloned().unwrap_or_default();
        let loader_keys: HashSet<String> = loader_libs.iter().filter_map(lib_key).collect();
        let mut libs = loader_libs;
        libs.extend(vanilla_libs.into_iter().filter(|l| lib_key(l).map_or(true, |k| !loader_keys.contains(&k))));
        obj.insert("libraries".into(), Value::Array(libs));

        if let Some(args) = loader.get("arguments").and_then(|a| a.as_object()) {
            let mut merged = obj.get("arguments").and_then(|a| a.as_object()).cloned().unwrap_or_default();
            for key in ["game", "jvm"] {
                if let Some(extra) = args.get(key).and_then(|v| v.as_array()) {
                    let mut list = merged.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    list.extend(extra.iter().cloned());
                    merged.insert(key.into(), Value::Array(list));
                }
            }
            obj.insert("arguments".into(), Value::Object(merged));
        }
        // 老版本加载器直接给完整的 minecraftArguments（带 tweakClass）
        if let Some(ma) = loader.get("minecraftArguments") {
            obj.insert("minecraftArguments".into(), ma.clone());
        }
        for key in ["releaseTime", "time"] {
            if let Some(v) = loader.get(key) {
                obj.insert(key.into(), v.clone());
            }
        }
    }
    out
}

/// 目录里已有的版本（versions/<名字>/<名字>.json 存在的）
pub fn installed_versions(root: &Path) -> Vec<InstalledVersion> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(root.join("versions")) else { return out };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let json_path = e.path().join(format!("{}.json", name));
        if !json_path.is_file() {
            continue;
        }
        let loader = std::fs::read_to_string(&json_path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| detect_loader(&v))
            .unwrap_or_default();
        out.push(InstalledVersion { name, loader });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledVersion {
    pub name: String,
    pub loader: String,
}

pub fn detect_loader(v: &Value) -> String {
    let main = v.get("mainClass").and_then(|m| m.as_str()).unwrap_or("");
    let libs = v.get("libraries").map(|l| l.to_string()).unwrap_or_default();
    if libs.contains("net.neoforged") {
        "NeoForge".into()
    } else if main.contains("fabricmc") {
        "Fabric".into()
    } else if main.contains("quiltmc") {
        "Quilt".into()
    } else if libs.contains("net.minecraftforge") || main.contains("cpw.mods") || main.contains("launchwrapper") {
        "Forge".into()
    } else if libs.contains("optifine") {
        "OptiFine".into()
    } else {
        String::new()
    }
}
