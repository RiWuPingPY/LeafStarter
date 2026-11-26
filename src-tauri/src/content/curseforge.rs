//! CurseForge：只连我们自己的代理（Key 在代理那边，客户端里没有）。两个代理：
//! - 源站 `api.pingyi.me:8443/ls/cf`（仓库 `server/ls-api.mjs`）：国内直连最快，优先用；
//! - 官网 Worker `leafstarter.pingyi.me/cf`（`site/src/cf.js`）：源站连不上、限速、在防护模式、还没放 Key 时改走它，
//!   之后 5 分钟都直接走 Worker，不再去试源站。
//! 设了环境变量 LEAFSTARTER_CF_PROXY 就只用它（调试用）。
//! 文件从 CurseForge 自己的 CDN 下载；作者关了第三方下载的文件没有下载地址，给项目页让用户自己下。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::install::Item;
use super::modrinth::{Hit, Query, VFile, Version};

const ORIGIN: &str = "https://api.pingyi.me:8443/ls/cf";
const WORKER: &str = "https://leafstarter.pingyi.me/cf";
/// 源站出问题以后多久不再试它：连不上 / 被断开（防护模式、封禁）5 分钟；只是上游出错、限速 1 分钟
const COOL_LONG: Duration = Duration::from_secs(300);
const COOL_SHORT: Duration = Duration::from_secs(60);
static ORIGIN_DOWN: Mutex<Option<Instant>> = Mutex::new(None);

fn custom() -> Option<String> {
    std::env::var("LEAFSTARTER_CF_PROXY").ok().filter(|s| !s.is_empty()).map(|s| s.trim_end_matches('/').to_string())
}

fn origin_ok() -> bool {
    ORIGIN_DOWN.lock().unwrap().is_none_or(|t| Instant::now() >= t)
}

/// 记下源站不可用，到这个时间再试
fn origin_down(d: Duration) {
    *ORIGIN_DOWN.lock().unwrap() = Some(Instant::now() + d);
}

/// 这次按顺序试哪几个代理
fn bases() -> Vec<String> {
    if let Some(c) = custom() {
        return vec![c];
    }
    if origin_ok() {
        vec![ORIGIN.into(), WORKER.into()]
    } else {
        vec![WORKER.into()]
    }
}

pub fn proxy() -> Option<String> {
    bases().into_iter().next()
}

/// 全程共用一个客户端：连接复用，第二次起省掉建连接和握手（实测每次搜索省 0.4～1 秒）。
/// 带上 Origin：源站的 nginx 会断开没有来源头的裸请求（防扫描）
fn http() -> reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::ORIGIN, reqwest::header::HeaderValue::from_static("https://leafstarter.pingyi.me"));
        reqwest::Client::builder()
            .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION")))
            .default_headers(h)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(90))
            .gzip(true)
            .build()
            .unwrap_or_default()
    })
    .clone()
}

/// 代理在不在、Key 配好没有（问 /health；结果记 10 分钟）。源站没准备好（没放 Key、防护模式）就记下来，直接看 Worker
pub async fn ready() -> bool {
    static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    if let Some((t, ok)) = *LAST.lock().unwrap() {
        if t.elapsed() < Duration::from_secs(600) {
            return ok;
        }
    }
    let mut ok = false;
    for base in bases() {
        let v: Option<Value> = async { http().get(format!("{base}/health")).timeout(Duration::from_secs(6)).send().await.ok()?.json().await.ok() }.await;
        let good = v.as_ref().is_some_and(|v| v["ready"].as_bool() == Some(true) && v["shield"].as_bool() != Some(true));
        if good {
            ok = true;
            break;
        }
        if base == ORIGIN {
            origin_down(COOL_LONG);
        }
    }
    *LAST.lock().unwrap() = Some((Instant::now(), ok));
    ok
}

/// 按顺序试代理：源站连不上、回 5xx / 429（没放 Key、限速、忙、防护模式断开）就记下来、换 Worker 再试一次
async fn send(make: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder) -> Result<Value> {
    let c = http();
    let list = bases();
    let mut last = None;
    for base in &list {
        let fallible = base == ORIGIN && list.len() > 1;
        match make(&c, base).send().await {
            Ok(r) if r.status().is_success() => return Ok(r.json().await?),
            Ok(r) if fallible && (r.status().is_server_error() || r.status().as_u16() == 429) => {
                origin_down(COOL_SHORT);
                last = Some(anyhow::anyhow!("CurseForge 返回 {}", r.status()));
            }
            Ok(r) => bail!("CurseForge 返回 {}", r.status()),
            Err(e) if fallible => {
                origin_down(COOL_LONG);
                last = Some(anyhow::Error::from(e));
            }
            Err(e) => return Err(e).context("连不上 CurseForge 代理"),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("CurseForge 还没接上"))).context("连不上 CurseForge 代理")
}

async fn get(path: &str) -> Result<Value> {
    send(|c, base| c.get(format!("{base}{path}"))).await
}

async fn post(path: &str, body: Value) -> Result<Value> {
    send(|c, base| c.post(format!("{base}{path}")).json(&body)).await
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
        zh: None,
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
