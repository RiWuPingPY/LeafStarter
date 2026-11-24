//! Modrinth（公开 API，不要 Key）：搜索、项目详情、版本、按 SHA-1 认出本地文件、检查更新。
//! 文档：https://docs.modrinth.com/api/ 。User-Agent 要能认出是哪个程序。

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const API: &str = "https://api.modrinth.com/v2";

pub fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION"), " (github.com/RiWuPingPY/LeafStarter)"))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .build()?)
}

async fn get(http: &reqwest::Client, url: &str) -> Result<Value> {
    let r = http.get(url).send().await.context("连不上 Modrinth")?;
    let status = r.status();
    if status.as_u16() == 429 {
        bail!("Modrinth 说请求太频繁了，等一分钟再试");
    }
    if !status.is_success() {
        bail!("Modrinth 返回 {status}");
    }
    Ok(r.json().await?)
}

async fn post(http: &reqwest::Client, url: &str, body: Value) -> Result<Value> {
    let r = http.post(url).json(&body).send().await.context("连不上 Modrinth")?;
    if !r.status().is_success() {
        bail!("Modrinth 返回 {}", r.status());
    }
    Ok(r.json().await?)
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// 搜索结果里的一项（字段名和界面一致，CurseForge 的也换成这个样子）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// modrinth / curseforge
    pub source: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    /// 支持的游戏版本（搜索结果里给的，可能不全）
    pub versions: Vec<String>,
    pub updated: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub text: String,
    /// mod / resourcepack / shader / modpack
    pub kind: String,
    pub mc: Option<String>,
    /// fabric / forge / neoforge / quilt（资源包、光影不看加载器）
    pub loader: Option<String>,
    /// relevance / downloads / updated
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
    /// modrinth（默认）/ curseforge
    #[serde(default)]
    pub source: String,
}

pub async fn search(http: &reqwest::Client, q: &Query) -> Result<(Vec<Hit>, u64)> {
    let mut facets = vec![json!([format!("project_type:{}", if q.kind.is_empty() { "mod" } else { &q.kind })])];
    if let Some(v) = q.mc.as_deref().filter(|v| !v.is_empty()) {
        facets.push(json!([format!("versions:{v}")]));
    }
    if let Some(l) = q.loader.as_deref().filter(|l| !l.is_empty() && matches!(q.kind.as_str(), "" | "mod" | "modpack")) {
        facets.push(json!([format!("categories:{}", l.to_lowercase())]));
    }
    let index = match q.sort.as_str() {
        "downloads" | "updated" | "newest" | "follows" => q.sort.as_str(),
        _ => "relevance",
    };
    let url = format!("{API}/search?query={}&facets={}&index={index}&limit=20&offset={}", enc(q.text.trim()), enc(&Value::Array(facets).to_string()), q.offset);
    let v = get(http, &url).await?;
    let hits = v["hits"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|h| Hit {
                    source: "modrinth".into(),
                    id: h["project_id"].as_str().unwrap_or("").into(),
                    slug: h["slug"].as_str().unwrap_or("").into(),
                    title: h["title"].as_str().unwrap_or("").into(),
                    description: h["description"].as_str().unwrap_or("").into(),
                    author: h["author"].as_str().unwrap_or("").into(),
                    downloads: h["downloads"].as_u64().unwrap_or(0),
                    icon: h["icon_url"].as_str().filter(|s| !s.is_empty()).map(String::from),
                    categories: serde_json::from_value(h["display_categories"].clone()).unwrap_or_default(),
                    versions: serde_json::from_value(h["versions"].clone()).unwrap_or_default(),
                    updated: h["date_modified"].as_str().unwrap_or("").into(),
                    url: format!("https://modrinth.com/{}/{}", h["project_type"].as_str().unwrap_or("mod"), h["slug"].as_str().unwrap_or("")),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((hits, v["total_hits"].as_u64().unwrap_or(0)))
}

/// 项目详情：介绍、链接
pub async fn project(http: &reqwest::Client, id: &str) -> Result<Value> {
    let v = get(http, &format!("{API}/project/{}", enc(id))).await?;
    Ok(json!({
        "source": "modrinth",
        "id": v["id"], "slug": v["slug"], "title": v["title"], "description": v["description"],
        "body": v["body"].as_str().map(|b| b.chars().take(6000).collect::<String>()),
        "icon": v["icon_url"], "downloads": v["downloads"], "license": v["license"]["id"],
        "clientSide": v["client_side"], "serverSide": v["server_side"], "kind": v["project_type"],
        "links": {
            "page": format!("https://modrinth.com/{}/{}", v["project_type"].as_str().unwrap_or("mod"), v["slug"].as_str().unwrap_or("")),
            "source": v["source_url"], "issues": v["issues_url"], "wiki": v["wiki_url"], "discord": v["discord_url"],
        },
        "gallery": v["gallery"].as_array().map(|g| g.iter().take(6).map(|x| x["url"].clone()).collect::<Vec<_>>()),
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VFile {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub sha1: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub number: String,
    /// release / beta / alpha
    pub kind: String,
    pub date: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub files: Vec<VFile>,
    /// (项目 id, 版本 id, 类型 required / optional / incompatible / embedded)
    pub deps: Vec<(Option<String>, Option<String>, String)>,
}

impl Version {
    pub fn primary(&self) -> Option<&VFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

pub fn parse_version(v: &Value) -> Version {
    Version {
        id: v["id"].as_str().unwrap_or("").into(),
        project_id: v["project_id"].as_str().unwrap_or("").into(),
        name: v["name"].as_str().unwrap_or("").into(),
        number: v["version_number"].as_str().unwrap_or("").into(),
        kind: v["version_type"].as_str().unwrap_or("release").into(),
        date: v["date_published"].as_str().unwrap_or("").into(),
        loaders: serde_json::from_value(v["loaders"].clone()).unwrap_or_default(),
        game_versions: serde_json::from_value(v["game_versions"].clone()).unwrap_or_default(),
        files: v["files"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|f| VFile {
                        url: f["url"].as_str().unwrap_or("").into(),
                        filename: f["filename"].as_str().unwrap_or("").into(),
                        size: f["size"].as_u64().unwrap_or(0),
                        sha1: f["hashes"]["sha1"].as_str().unwrap_or("").into(),
                        primary: f["primary"].as_bool().unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        deps: v["dependencies"]
            .as_array()
            .map(|a| a.iter().map(|d| (d["project_id"].as_str().map(String::from), d["version_id"].as_str().map(String::from), d["dependency_type"].as_str().unwrap_or("optional").to_string())).collect())
            .unwrap_or_default(),
    }
}

/// 一个项目适合这个实例的版本，新的在前。资源包、光影不传加载器
pub async fn versions(http: &reqwest::Client, id: &str, mc: Option<&str>, loader: Option<&str>) -> Result<Vec<Version>> {
    let mut url = format!("{API}/project/{}/version?include_changelog=false", enc(id));
    if let Some(l) = loader.filter(|l| !l.is_empty()) {
        // NeoForge 1.20.1 上也能跑一部分 Forge 的 Mod，但不一定；只按实例本身的加载器过滤
        url.push_str(&format!("&loaders={}", enc(&json!([l.to_lowercase()]).to_string())));
    }
    if let Some(v) = mc.filter(|v| !v.is_empty()) {
        url.push_str(&format!("&game_versions={}", enc(&json!([v]).to_string())));
    }
    let v = get(http, &url).await?;
    Ok(v.as_array().map(|a| a.iter().map(parse_version).collect()).unwrap_or_default())
}

pub async fn version(http: &reqwest::Client, id: &str) -> Result<Version> {
    Ok(parse_version(&get(http, &format!("{API}/version/{}", enc(id))).await?))
}

/// 挑一个：正式版优先，没有正式版就用最新的测试版
pub fn pick(list: &[Version]) -> Option<&Version> {
    list.iter().find(|v| v.kind == "release").or_else(|| list.first())
}

/// 按 SHA-1 认出文件属于哪个项目的哪个版本
pub async fn by_hashes(http: &reqwest::Client, sha1s: &[String]) -> Result<HashMap<String, Version>> {
    if sha1s.is_empty() {
        return Ok(HashMap::new());
    }
    let v = post(http, &format!("{API}/version_files"), json!({ "hashes": sha1s, "algorithm": "sha1" })).await?;
    Ok(v.as_object().map(|m| m.iter().map(|(k, x)| (k.clone(), parse_version(x))).collect()).unwrap_or_default())
}

/// 检查更新：每个文件在这个游戏版本和加载器下最新的版本（和现在一样就说明不用更新）
pub async fn updates(http: &reqwest::Client, sha1s: &[String], mc: &str, loader: Option<&str>) -> Result<HashMap<String, Version>> {
    if sha1s.is_empty() {
        return Ok(HashMap::new());
    }
    let mut body = json!({ "hashes": sha1s, "algorithm": "sha1", "game_versions": [mc] });
    if let Some(l) = loader.filter(|l| !l.is_empty()) {
        body["loaders"] = json!([l.to_lowercase()]);
    }
    let v = post(http, &format!("{API}/version_files/update"), body).await?;
    Ok(v.as_object().map(|m| m.iter().map(|(k, x)| (k.clone(), parse_version(x))).collect()).unwrap_or_default())
}

/// 一批项目的名字和图标
pub async fn projects(http: &reqwest::Client, ids: &[String]) -> Result<HashMap<String, Value>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let v = get(http, &format!("{API}/projects?ids={}", enc(&json!(ids).to_string()))).await?;
    Ok(v.as_array().map(|a| a.iter().filter_map(|p| Some((p["id"].as_str()?.to_string(), json!({ "title": p["title"], "slug": p["slug"], "icon": p["icon_url"], "kind": p["project_type"] })))).collect()).unwrap_or_default())
}
