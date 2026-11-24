//! 低配优化包（ROADMAP 5.8）：按「加载器 + 游戏版本」从 Modrinth 挑能装的优化 Mod，
//! 一键加上；撤掉时只删我们加的那几个（记在实例设置里）。

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use super::install::{self, Item};
use super::modrinth;

/// (Modrinth 上的 slug, 名字, 做什么)
const FABRIC: &[(&str, &str, &str)] = &[
    ("sodium", "Sodium", "重写渲染，帧数通常翻倍"),
    ("lithium", "Lithium", "优化游戏逻辑（生物、方块更新）"),
    ("ferrite-core", "FerriteCore", "省内存"),
    ("immediatelyfast", "ImmediatelyFast", "优化界面、文字、地图的绘制"),
    ("entityculling", "Entity Culling", "看不见的生物和方块实体不画"),
    ("modernfix", "ModernFix", "加快启动、省内存"),
    ("dynamic-fps", "Dynamic FPS", "切到后台时降低帧数，省电不发烫"),
];
const FORGE: &[(&str, &str, &str)] = &[
    ("embeddium", "Embeddium", "Forge 上的 Sodium，帧数通常翻倍"),
    ("ferrite-core", "FerriteCore", "省内存"),
    ("immediatelyfast", "ImmediatelyFast", "优化界面、文字、地图的绘制"),
    ("entityculling", "Entity Culling", "看不见的生物和方块实体不画"),
    ("modernfix", "ModernFix", "加快启动、省内存"),
    ("dynamic-fps", "Dynamic FPS", "切到后台时降低帧数，省电不发烫"),
];
const NEOFORGE: &[(&str, &str, &str)] = &[
    ("sodium", "Sodium", "重写渲染，帧数通常翻倍"),
    ("ferrite-core", "FerriteCore", "省内存"),
    ("immediatelyfast", "ImmediatelyFast", "优化界面、文字、地图的绘制"),
    ("entityculling", "Entity Culling", "看不见的生物和方块实体不画"),
    ("modernfix", "ModernFix", "加快启动、省内存"),
    ("dynamic-fps", "Dynamic FPS", "切到后台时降低帧数，省电不发烫"),
];

/// 和渲染优化冲突的（装了这些就不加 Sodium / Embeddium）
const RENDER_CONFLICT: &[&str] = &["optifine", "optifabric", "rubidium", "embeddium", "sodium", "magnesium"];

pub fn list_for(loader: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match loader.to_lowercase().as_str() {
        "fabric" | "quilt" => FABRIC,
        "forge" => FORGE,
        "neoforge" => NEOFORGE,
        _ => &[],
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub slug: String,
    pub name: String,
    pub what: String,
    /// ok：能加；have：已经有了；none：这个版本没有；conflict：和已有的冲突
    pub state: String,
    pub note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,
}

/// 看看这个实例能加哪些
pub async fn check(http: &reqwest::Client, mods_dir: &Path, mc: &str, loader: &str) -> Result<Vec<Entry>> {
    let local = install::local_hashes(mods_dir);
    let known = modrinth::by_hashes(http, &local.keys().cloned().collect::<Vec<_>>()).await.unwrap_or_default();
    let have_projects: Vec<String> = known.values().map(|v| v.project_id.clone()).collect();
    // 本地文件名里的 id（OptiFine 这类不在 Modrinth 上的也要认）
    let names: Vec<String> = local.values().map(|n| n.to_lowercase()).collect();
    let metas = crate::game::modmeta::read_dir(mods_dir, loader);
    let ids: Vec<String> = metas.iter().filter(|m| m.enabled).filter_map(|m| m.id.clone()).map(|s| s.to_lowercase()).collect();
    let mut out = vec![];
    let mut pids: HashMap<String, String> = HashMap::new();
    for (slug, name, what) in list_for(loader) {
        let mut e = Entry { slug: slug.to_string(), name: name.to_string(), what: what.to_string(), state: "ok".into(), note: String::new(), item: None };
        let render = matches!(*slug, "sodium" | "embeddium");
        if ids.iter().any(|i| i == slug || i.replace('_', "-") == *slug) || names.iter().any(|n| n.starts_with(slug)) {
            e.state = "have".into();
            out.push(e);
            continue;
        }
        if render {
            if let Some(c) = RENDER_CONFLICT.iter().find(|c| ids.iter().any(|i| i == *c) || names.iter().any(|n| n.contains(*c))) {
                e.state = "conflict".into();
                e.note = format!("已经有 {c}，两个渲染优化不能一起装");
                out.push(e);
                continue;
            }
        }
        let list = modrinth::versions(http, slug, Some(mc), Some(loader)).await.unwrap_or_default();
        match modrinth::pick(&list) {
            Some(v) => {
                if have_projects.contains(&v.project_id) {
                    e.state = "have".into();
                } else if let Some(f) = v.primary().cloned() {
                    pids.insert(v.project_id.clone(), slug.to_string());
                    e.item = Some(Item { project: v.project_id.clone(), title: name.to_string(), version: v.number.clone(), file: f, dep: false, replaces: None });
                }
            }
            None => {
                e.state = "none".into();
                e.note = format!("没有 {mc} {loader} 的版本");
            }
        }
        out.push(e);
    }
    Ok(out)
}
