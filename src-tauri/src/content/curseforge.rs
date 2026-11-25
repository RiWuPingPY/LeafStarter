//! CurseForge：只连我们自己的代理（Key 在代理那边，客户端里没有），代理代码在仓库的 `site/src/cf.js`（官网的 Worker）。
//! 代理地址还没部署时（`PROXY` 为空、也没设环境变量 LEAFSTARTER_CF_PROXY）界面上说明，只用 Modrinth。
//! 文件从 CurseForge 自己的 CDN 下载；作者关了第三方下载的文件没有下载地址，给项目页让用户自己下。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::install::Item;
use super::modrinth::{Hit, Query, VFile, Version};

/// 代理在官网的 Worker 里（site/src/cf.js），Key 在那边的密钥里
const PROXY: &str = "https://leafstarter.pingyi.me/cf";

pub fn proxy() -> Option<String> {
    std::env::var("LEAFSTARTER_CF_PROXY").ok().filter(|s| !s.is_empty()).or_else(|| Some(PROXY.to_string()).filter(|s| !s.is_empty())).map(|s| s.trim_end_matches('/').to_string())
}

/// 代理在不在、Key 配好没有（问 /health；结果记 10 分钟）
pub async fn ready() -> bool {
    use std::sync::Mutex;
    static LAST: Mutex<Option<(std::time::Instant, bool)>> = Mutex::new(None);
    if let Some((t, ok)) = *LAST.lock().unwrap() {
        if t.elapsed() < Duration::from_secs(600) {
            return ok;
        }
    }
    let ok = async {
        let (c, base) = client().ok()?;
        let v: Value = c.get(format!("{base}/health")).timeout(Duration::from_secs(6)).send().await.ok()?.json().await.ok()?;
        v["ready"].as_bool()
    }
    .await
    .unwrap_or(false);
    *LAST.lock().unwrap() = Some((std::time::Instant::now(), ok));
    ok
}

fn client() -> Result<(reqwest::Client, String)> {
    let base = proxy().context("CurseForge 还没接上：代理还没部署")?;
    let c = reqwest::Client::builder().user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION"))).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(30)).build()?;
    Ok((c, base))
}

async fn get(path: &str) -> Result<Value> {
    let (c, base) = client()?;
    let r = c.get(format!("{base}{path}")).send().await.context("连不上 CurseForge 代理")?;
    if !r.status().is_success() {
        bail!("CurseForge 返回 {}", r.status());
    }
    Ok(r.json().await?)
}

async fn post(path: &str, body: Value) -> Result<Value> {
    let (c, base) = client()?;
    let r = c.post(format!("{base}{path}")).json(&body).send().await.context("连不上 CurseForge 代理")?;
    if !r.status().is_success() {
        bail!("CurseForge 返回 {}", r.status());
    }
    Ok(r.json().await?)
}

fn class_of(kind: &str) -> u32 {
    match kind {
        "resourcepack" => 12,
        "shader" => 6552,
        "modpack" => 4471,
        _ => 6,
    }
}

fn loader_type(l: &str) -> Option<u32> {
    match l.to_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn hit(m: &Value) -> Hit {
    Hit {
        source: "curseforge".into(),
        id: m["id"].as_u64().unwrap_or(0).to_string(),
        slug: m["slug"].as_str().unwrap_or("").into(),
        title: m["name"].as_str().unwrap_or("").into(),
        description: m["summary"].as_str().unwrap_or("").into(),
        author: m["authors"][0]["name"].as_str().unwrap_or("").into(),
        downloads: m["downloadCount"].as_f64().unwrap_or(0.0) as u64,
        icon: m["logo"]["thumbnailUrl"].as_str().filter(|s| !s.is_empty()).map(String::from),
        categories: m["categories"].as_array().map(|a| a.iter().filter_map(|c| c["name"].as_str().map(String::from)).collect()).unwrap_or_default(),
        versions: m["latestFilesIndexes"].as_array().map(|a| a.iter().filter_map(|x| x["gameVersion"].as_str().map(String::from)).collect()).unwrap_or_default(),
        updated: m["dateModified"].as_str().unwrap_or("").into(),
        url: m["links"]["websiteUrl"].as_str().unwrap_or("").into(),
    }
}

pub async fn search(q: &Query) -> Result<(Vec<Hit>, u64)> {
    let mut path = format!("/v1/mods/search?classId={}&pageSize=20&index={}&sortOrder=desc", class_of(&q.kind), q.offset);
    path.push_str(&format!("&sortField={}", match q.sort.as_str() { "downloads" => 6, "updated" => 3, _ => 2 }));
    if !q.text.trim().is_empty() {
        path.push_str(&format!("&searchFilter={}", urlenc(q.text.trim())));
    }
    if let Some(v) = q.mc.as_deref().filter(|v| !v.is_empty()) {
        path.push_str(&format!("&gameVersion={}", urlenc(v)));
    }
    if let Some(t) = q.loader.as_deref().and_then(loader_type) {
        path.push_str(&format!("&modLoaderType={t}"));
    }
    let v = get(&path).await?;
    let hits = v["data"].as_array().map(|a| a.iter().map(hit).collect()).unwrap_or_default();
    Ok((hits, v["pagination"]["totalCount"].as_u64().unwrap_or(0).min(10_000)))
}

pub async fn project(id: &str) -> Result<Value> {
    let m = get(&format!("/v1/mods/{}", num(id)?)).await?["data"].clone();
    Ok(json!({
        "source": "curseforge", "id": id, "slug": m["slug"], "title": m["name"], "description": m["summary"],
        "icon": m["logo"]["thumbnailUrl"], "downloads": m["downloadCount"],
        "links": { "page": m["links"]["websiteUrl"], "source": m["links"]["sourceUrl"], "issues": m["links"]["issuesUrl"], "wiki": m["links"]["wikiUrl"] },
    }))
}

fn version(mod_id: &str, f: &Value) -> Version {
    let kind = match f["releaseType"].as_u64() {
        Some(1) => "release",
        Some(2) => "beta",
        _ => "alpha",
    };
    let sha1 = f["hashes"].as_array().and_then(|a| a.iter().find(|h| h["algo"].as_u64() == Some(1))).and_then(|h| h["value"].as_str()).unwrap_or("").to_string();
    let gv: Vec<String> = f["gameVersions"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    Version {
        id: f["id"].as_u64().unwrap_or(0).to_string(),
        project_id: mod_id.to_string(),
        name: f["displayName"].as_str().unwrap_or("").into(),
        number: f["displayName"].as_str().unwrap_or("").into(),
        kind: kind.into(),
        date: f["fileDate"].as_str().unwrap_or("").into(),
        loaders: gv.iter().filter(|x| loader_type(x).is_some()).map(|x| x.to_lowercase()).collect(),
        game_versions: gv.iter().filter(|x| loader_type(x).is_none()).cloned().collect(),
        files: vec![VFile {
            // 作者关了第三方下载时 downloadUrl 是 null：留空，装的时候提示去项目页下
            url: f["downloadUrl"].as_str().unwrap_or("").into(),
            filename: f["fileName"].as_str().unwrap_or("").into(),
            size: f["fileLength"].as_u64().unwrap_or(0),
            sha1,
            primary: true,
        }],
        deps: f["dependencies"].as_array().map(|a| a.iter().map(|d| (d["modId"].as_u64().map(|x| x.to_string()), None, if d["relationType"].as_u64() == Some(3) { "required".to_string() } else { "optional".to_string() })).collect()).unwrap_or_default(),
    }
}

pub async fn versions(id: &str, mc: Option<&str>, loader: Option<&str>) -> Result<Vec<Version>> {
    let mut path = format!("/v1/mods/{}/files?pageSize=50", num(id)?);
    if let Some(v) = mc.filter(|v| !v.is_empty()) {
        path.push_str(&format!("&gameVersion={}", urlenc(v)));
    }
    if let Some(t) = loader.and_then(loader_type) {
        path.push_str(&format!("&modLoaderType={t}"));
    }
    let v = get(&path).await?;
    let mut list: Vec<Version> = v["data"].as_array().map(|a| a.iter().map(|f| version(id, f)).collect()).unwrap_or_default();
    list.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(list)
}

/// 装之前算：选中的版本 + 必需前置（按实例的游戏版本和加载器挑最新的）
pub async fn plan(id: &str, version_id: Option<&str>, mc: Option<&str>, loader: Option<&str>, installed: &[String]) -> Result<Vec<Item>> {
    let list = versions(id, mc, loader).await?;
    let first = match version_id {
        Some(v) => list.iter().find(|x| x.id == v).cloned().context("找不到这个版本")?,
        None => super::modrinth::pick(&list).cloned().context("这个项目没有适合这个实例的版本")?,
    };
    let mut out = vec![];
    let mut queue = vec![(first, false)];
    let mut seen = std::collections::HashSet::new();
    while let Some((v, dep)) = queue.pop() {
        if !seen.insert(v.project_id.clone()) {
            continue;
        }
        let f = v.files[0].clone();
        if f.url.is_empty() {
            bail!("「{}」的作者关闭了第三方下载，要到 CurseForge 项目页手动下载，下好放进实例的文件夹", f.filename);
        }
        for (pid, _, kind) in &v.deps {
            let Some(pid) = pid else { continue };
            if kind == "required" && !installed.contains(pid) && !seen.contains(pid) {
                if let Some(d) = super::modrinth::pick(&versions(pid, mc, loader).await?).cloned() {
                    queue.push((d, true));
                }
            }
        }
        out.push(Item { project: v.project_id.clone(), title: String::new(), version: v.number.clone(), file: f, dep, replaces: None });
    }
    Ok(out)
}

/// CurseForge 认文件用的指纹：去掉空白字节（9、10、13、32）后的 MurmurHash2（种子 1）
pub fn fingerprint(data: &[u8]) -> u32 {
    let b: Vec<u8> = data.iter().copied().filter(|c| !matches!(c, 9 | 10 | 13 | 32)).collect();
    murmur2(&b, 1)
}

fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    let r = chunks.remainder();
    if r.len() >= 3 {
        h ^= (r[2] as u32) << 16;
    }
    if r.len() >= 2 {
        h ^= (r[1] as u32) << 8;
    }
    if !r.is_empty() {
        h ^= r[0] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// 按指纹认出本地文件：指纹 → 项目 id
pub async fn by_fingerprints(fps: &[u32]) -> Result<std::collections::HashMap<u32, String>> {
    let v = post("/v1/fingerprints/432", json!({ "fingerprints": fps })).await?;
    Ok(v["data"]["exactMatches"].as_array().map(|a| a.iter().filter_map(|m| Some((m["file"]["fileFingerprint"].as_u64()? as u32, m["id"].as_u64()?.to_string()))).collect()).unwrap_or_default())
}

/// 一批文件的信息：(项目 id, 文件名, 下载地址（作者关了第三方下载时为空）, SHA-1, 大小)
pub async fn files(ids: &[u64]) -> Result<Vec<(u64, String, String, Option<String>, Option<u64>)>> {
    let mut out = vec![];
    for chunk in ids.chunks(200) {
        let v = post("/v1/mods/files", json!({ "fileIds": chunk })).await?;
        for f in v["data"].as_array().into_iter().flatten() {
            let sha1 = f["hashes"].as_array().and_then(|a| a.iter().find(|h| h["algo"].as_u64() == Some(1))).and_then(|h| h["value"].as_str()).map(String::from);
            out.push((f["modId"].as_u64().unwrap_or(0), f["fileName"].as_str().unwrap_or("").to_string(), f["downloadUrl"].as_str().unwrap_or("").to_string(), sha1, f["fileLength"].as_u64()));
        }
    }
    Ok(out)
}

/// 一批项目是什么类型（6 Mod、12 资源包、6552 光影）
pub async fn classes(mod_ids: &[u64]) -> Result<std::collections::HashMap<u64, u64>> {
    let mut out = std::collections::HashMap::new();
    for chunk in mod_ids.chunks(200) {
        let v = post("/v1/mods", json!({ "modIds": chunk })).await?;
        for m in v["data"].as_array().into_iter().flatten() {
            if let (Some(id), Some(c)) = (m["id"].as_u64(), m["classId"].as_u64()) {
                out.insert(id, c);
            }
        }
    }
    Ok(out)
}

fn num(id: &str) -> Result<u64> {
    id.parse().map_err(|_| anyhow::anyhow!("CurseForge 项目 id 不对"))
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn murmur2_known_values() {
        // 公开的 MurmurHash2（种子 1）参考值
        assert_eq!(murmur2(b"", 1), 0x5bd1_5e36);
        assert_eq!(fingerprint(b"a b\tc\r\n"), murmur2(b"abc", 1));
    }
}
