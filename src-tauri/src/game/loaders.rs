//! 加载器版本列表：Fabric / Quilt / Forge / NeoForge。

use std::cmp::Ordering;
use std::collections::HashMap;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::base::net::{Net, BMCLAPI_BASE};

pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/";
pub const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    /// 安装时回传的标识：Fabric/Quilt 是版本号，Forge/NeoForge 是安装器在 Maven 里的路径
    pub id: String,
    pub version: String,
    pub stable: bool,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub fn parse(s: &str) -> Option<LoaderKind> {
        match s.to_ascii_lowercase().as_str() {
            "fabric" => Some(LoaderKind::Fabric),
            "quilt" => Some(LoaderKind::Quilt),
            "forge" => Some(LoaderKind::Forge),
            "neoforge" => Some(LoaderKind::NeoForge),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
            LoaderKind::Forge => "Forge",
            LoaderKind::NeoForge => "NeoForge",
        }
    }
    pub fn needs_installer(self) -> bool {
        matches!(self, LoaderKind::Forge | LoaderKind::NeoForge)
    }
}

/// 按数字段比较版本号："21.1.77" > "21.1.9"，"26.1.0.1-beta" 也能比
pub fn version_cmp(a: &str, b: &str) -> Ordering {
    fn nums(s: &str) -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().unwrap_or(0)).collect()
    }
    nums(a).cmp(&nums(b))
}

pub async fn list(net: &Net, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    let list = match kind {
        LoaderKind::Fabric => fabric_like(net, &format!("https://meta.fabricmc.net/v2/versions/loader/{}", mc)).await?,
        LoaderKind::Quilt => fabric_like(net, &format!("https://meta.quiltmc.org/v3/versions/loader/{}", mc)).await?,
        LoaderKind::Forge => forge(net, mc).await?,
        LoaderKind::NeoForge => neoforge(net, mc).await?,
    };
    Ok(list)
}

async fn fabric_like(net: &Net, url: &str) -> Result<Vec<LoaderVersion>> {
    let items: Vec<Value> = net.get_json(url).await?;
    let mut out: Vec<LoaderVersion> = items
        .iter()
        .filter_map(|it| {
            let loader = it.get("loader")?;
            let v = loader.get("version")?.as_str()?.to_string();
            let stable = loader
                .get("stable")
                .and_then(|s| s.as_bool())
                .unwrap_or_else(|| !v.contains("beta") && !v.contains("alpha") && !v.contains("pre"));
            Some(LoaderVersion { id: v.clone(), version: v, stable, tag: None })
        })
        .collect();
    out.sort_by(|a, b| version_cmp(&b.version, &a.version));
    if let Some(first) = out.iter_mut().find(|v| v.stable) {
        first.tag = Some("推荐".into());
    }
    Ok(out)
}

fn prefer_bmclapi(net: &Net) -> bool {
    net.best_source_id() != "official"
}

#[derive(Deserialize)]
struct BmclForge {
    version: String,
    mcversion: String,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    build: u64,
    #[serde(default)]
    files: Vec<BmclForgeFile>,
}

#[derive(Deserialize)]
struct BmclForgeFile {
    #[serde(default)]
    category: String,
}

async fn forge(net: &Net, mc: &str) -> Result<Vec<LoaderVersion>> {
    let from_bmcl = || async {
        let items: Vec<BmclForge> = net.get_json_from(&[format!("{}/forge/minecraft/{}", BMCLAPI_BASE, mc)]).await?;
        let mut out: Vec<(u64, LoaderVersion)> = items
            .into_iter()
            .filter(|f| f.files.iter().any(|x| x.category == "installer"))
            .map(|f| {
                let mut full = format!("{}-{}", f.mcversion, f.version);
                if let Some(b) = f.branch.filter(|b| !b.is_empty()) {
                    full = format!("{}-{}", full, b);
                }
                let id = format!("net/minecraftforge/forge/{0}/forge-{0}-installer.jar", full);
                (f.build, LoaderVersion { id, version: f.version, stable: true, tag: None })
            })
            .collect();
        out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| version_cmp(&b.1.version, &a.1.version)));
        anyhow::Ok(out.into_iter().map(|x| x.1).collect::<Vec<_>>())
    };
    let from_official = || async {
        let meta: HashMap<String, Vec<String>> = net
            .get_json_from(&["https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json".to_string()])
            .await?;
        let fulls = meta.get(mc).cloned().unwrap_or_default();
        let mut out: Vec<LoaderVersion> = fulls
            .into_iter()
            .map(|full| {
                let short = full
                    .strip_prefix(&format!("{}-", mc))
                    .unwrap_or(&full)
                    .trim_end_matches(&format!("-{}", mc))
                    .to_string();
                let id = format!("net/minecraftforge/forge/{0}/forge-{0}-installer.jar", full);
                LoaderVersion { id, version: short, stable: true, tag: None }
            })
            .collect();
        out.sort_by(|a, b| version_cmp(&b.version, &a.version));
        anyhow::Ok(out)
    };
    let mut list = if prefer_bmclapi(net) {
        match from_bmcl().await {
            Ok(l) if !l.is_empty() => l,
            _ => from_official().await?,
        }
    } else {
        match from_official().await {
            Ok(l) if !l.is_empty() => l,
            _ => from_bmcl().await?,
        }
    };
    if list.is_empty() {
        bail!("Minecraft {} 没有可用的 Forge", mc);
    }
    // 官方的推荐 / 最新标记，拿不到就算了
    if let Ok(promos) = net
        .get_json_from::<Value>(&["https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json".to_string()])
        .await
    {
        let p = promos.get("promos");
        let rec = p.and_then(|p| p.get(format!("{}-recommended", mc))).and_then(|v| v.as_str()).map(String::from);
        let latest = p.and_then(|p| p.get(format!("{}-latest", mc))).and_then(|v| v.as_str()).map(String::from);
        for v in list.iter_mut() {
            if Some(&v.version) == rec.as_ref() {
                v.tag = Some("推荐".into());
            } else if Some(&v.version) == latest.as_ref() {
                v.tag = Some("最新".into());
            }
        }
    }
    if list.iter().all(|v| v.tag.is_none()) {
        list[0].tag = Some("最新".into());
    }
    Ok(list)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BmclNeo {
    version: String,
    installer_path: String,
}

/// NeoForge 版本号 → Minecraft 版本："21.1.77" → "1.21.1"，"26.1.0.5-beta" → "26.1"
pub fn neoforge_mc(v: &str) -> Option<String> {
    let core = v.split('-').next()?;
    let parts: Vec<&str> = core.split('.').collect();
    let major: u32 = parts.first()?.parse().ok()?;
    if major >= 26 && parts.len() >= 4 {
        let minor = parts[1];
        let patch = parts[2];
        Some(if patch == "0" { format!("{}.{}", major, minor) } else { format!("{}.{}.{}", major, minor, patch) })
    } else if parts.len() >= 2 {
        let minor = parts[1];
        Some(if minor == "0" { format!("1.{}", major) } else { format!("1.{}.{}", major, minor) })
    } else {
        None
    }
}

fn xml_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|s| s.split("</version>").next())
        .map(|s| s.trim().to_string())
        .collect()
}

async fn neoforge(net: &Net, mc: &str) -> Result<Vec<LoaderVersion>> {
    let from_bmcl = || async {
        let items: Vec<BmclNeo> = net.get_json_from(&[format!("{}/neoforge/list/{}", BMCLAPI_BASE, mc)]).await?;
        anyhow::Ok(
            items
                .into_iter()
                .map(|n| LoaderVersion {
                    id: n.installer_path.trim_start_matches('/').trim_start_matches("maven/").to_string(),
                    stable: !n.version.contains("beta") && !n.version.contains("alpha"),
                    version: n.version,
                    tag: None,
                })
                .collect::<Vec<_>>(),
        )
    };
    let from_official = || async {
        if mc == "1.20.1" {
            // 1.20.1 时期 NeoForge 还叫 forge，坐标是 net.neoforged:forge
            let xml = net
                .get_text_from(&[format!("{}net/neoforged/forge/maven-metadata.xml", NEOFORGE_MAVEN)])
                .await?;
            return anyhow::Ok(
                xml_versions(&xml)
                    .into_iter()
                    .filter(|v| v.starts_with("1.20.1-"))
                    .map(|v| LoaderVersion {
                        id: format!("net/neoforged/forge/{0}/forge-{0}-installer.jar", v),
                        version: v.trim_start_matches("1.20.1-").to_string(),
                        stable: true,
                        tag: None,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let xml = net
            .get_text_from(&[format!("{}net/neoforged/neoforge/maven-metadata.xml", NEOFORGE_MAVEN)])
            .await?;
        anyhow::Ok(
            xml_versions(&xml)
                .into_iter()
                .filter(|v| neoforge_mc(v).as_deref() == Some(mc))
                .map(|v| LoaderVersion {
                    id: format!("net/neoforged/neoforge/{0}/neoforge-{0}-installer.jar", v),
                    stable: !v.contains("beta") && !v.contains("alpha"),
                    version: v,
                    tag: None,
                })
                .collect::<Vec<_>>(),
        )
    };
    let mut list = if prefer_bmclapi(net) {
        match from_bmcl().await {
            Ok(l) if !l.is_empty() => l,
            _ => from_official().await?,
        }
    } else {
        match from_official().await {
            Ok(l) if !l.is_empty() => l,
            _ => from_bmcl().await?,
        }
    };
    if list.is_empty() {
        bail!("Minecraft {} 没有可用的 NeoForge", mc);
    }
    list.sort_by(|a, b| version_cmp(&b.version, &a.version));
    if let Some(first) = list.iter_mut().find(|v| v.stable) {
        first.tag = Some("推荐".into());
    } else {
        list[0].tag = Some("最新".into());
    }
    Ok(list)
}

/// 安装器 jar 的官方下载地址
pub fn installer_url(kind: LoaderKind, id: &str) -> String {
    match kind {
        LoaderKind::NeoForge => format!("{}{}", NEOFORGE_MAVEN, id),
        _ => format!("{}{}", FORGE_MAVEN, id),
    }
}

/// Fabric / Quilt 的版本 JSON（profile）
pub async fn fabric_profile(net: &Net, kind: LoaderKind, mc: &str, version: &str) -> Result<Value> {
    let url = match kind {
        LoaderKind::Fabric => format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json", mc, version),
        LoaderKind::Quilt => format!("https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json", mc, version),
        _ => bail!("不是 Fabric/Quilt"),
    };
    net.get_json(&url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_mapping() {
        assert_eq!(neoforge_mc("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_mc("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_mc("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_mc("26.1.0.1-beta").as_deref(), Some("26.1"));
        assert_eq!(neoforge_mc("26.3.0.95").as_deref(), Some("26.3"));
        assert_eq!(neoforge_mc("26.1.2.3").as_deref(), Some("26.1.2"));
    }

    #[test]
    fn version_order() {
        assert_eq!(version_cmp("21.1.77", "21.1.9"), Ordering::Greater);
        assert_eq!(version_cmp("0.16.10", "0.16.9"), Ordering::Greater);
    }
}
