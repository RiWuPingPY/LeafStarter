//! 用中文名搜 Mod：搜「苹果皮」也能出 AppleSkin。
//! 1. 先查自带的一小份常用 Mod 中文名。MC百科的搜索对附属很多的大 Mod 排得不好
//!    （搜「机械动力」第一页全是附属，没有机械动力本体），这些靠自带的名字兜住；
//! 2. 再问 MC百科的搜索页，从结果标题「[缩写] 中文名 (English Name)」里拿英文名。
//! 拿到英文名以后由调用方去 Modrinth / CurseForge 搜，只认名字对得上的那一个。

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use super::modrinth::Hit;

#[derive(Debug, Clone, PartialEq)]
pub struct Name {
    pub zh: String,
    pub en: String,
    /// 自带名字里记的 slug（Modrinth / CurseForge 一般一样），MC百科来的没有
    pub slug: String,
}

/// 常用 Mod 的中文名：「中文名|别名」、英文名、slug
const DICT: &[(&str, &str, &str)] = &[
    ("钠", "Sodium", "sodium"),
    ("锂", "Lithium", "lithium"),
    ("实体剔除", "Entity Culling", "entityculling"),
    ("模组菜单", "Mod Menu", "modmenu"),
    ("JEI物品管理器|物品管理器", "Just Enough Items", "jei"),
    ("REI物品管理器", "Roughly Enough Items", "rei"),
    ("Xaero的小地图|小地图", "Xaero's Minimap", "xaeros-minimap"),
    ("Xaero的世界地图|世界地图", "Xaero's World Map", "xaeros-world-map"),
    ("旅行地图", "JourneyMap", "journeymap"),
    ("苹果皮", "AppleSkin", "appleskin"),
    ("投影", "Litematica", "litematica"),
    ("动态光源", "LambDynamicLights", "lambdynamiclights"),
    ("简单语音聊天|语音聊天", "Simple Voice Chat", "simple-voice-chat"),
    ("传送石碑", "Waystones", "waystones"),
    ("精妙背包", "Sophisticated Backpacks", "sophisticated-backpacks"),
    ("精妙存储", "Sophisticated Storage", "sophisticated-storage"),
    ("旅行者背包", "Traveler's Backpack", "travelersbackpack"),
    ("铁箱子", "Iron Chests", "iron-chests"),
    ("储物抽屉|存储抽屉", "Storage Drawers", "storage-drawers"),
    ("精致存储", "Refined Storage", "refined-storage"),
    ("应用能源2|应用能源", "Applied Energistics 2", "ae2"),
    ("机械动力", "Create", "create"),
    // Fabric 版是另一个项目
    ("机械动力", "Create Fabric", "create-fabric"),
    ("农夫乐事", "Farmer's Delight", "farmers-delight"),
    ("植物魔法", "Botania", "botania"),
    ("匠魂", "Tinkers' Construct", "tinkers-construct"),
    ("热力膨胀", "Thermal Expansion", "thermal-expansion"),
    ("热力基本", "Thermal Foundation", "thermal-foundation"),
    ("通用机械", "Mekanism", "mekanism"),
    ("工业时代2|工业2", "IndustrialCraft 2", "industrial-craft"),
    ("沉浸工程", "Immersive Engineering", "immersiveengineering"),
    ("神秘时代", "Thaumcraft", "thaumcraft"),
    ("暮色森林", "The Twilight Forest", "the-twilight-forest"),
    ("冰火传说|冰与火之歌", "Ice and Fire", "ice-and-fire-dragons"),
    ("Alex的生物", "Alex's Mobs", "alexs-mobs"),
    ("Alex的洞穴", "Alex's Caves", "alexs-caves"),
    ("超多生物群系", "Biomes O' Plenty", "biomes-o-plenty"),
    ("夸克", "Quark", "quark"),
    ("凿子", "Chisel", "chisel"),
    ("龙之进化", "Draconic Evolution", "draconic-evolution"),
    ("末影接口", "Ender IO", "ender-io"),
    ("末影存储", "Ender Storage", "ender-storage"),
    ("林业", "Forestry", "forestry"),
    ("星系", "Galacticraft", "galacticraft"),
    ("神秘农业", "Mystical Agriculture", "mystical-agriculture"),
    ("气动工艺", "PneumaticCraft", "pneumaticcraft-repressurized"),
    ("血魔法", "Blood Magic", "blood-magic"),
    ("星辉魔法", "Astral Sorcery", "astral-sorcery"),
    ("新生魔艺", "Ars Nouveau", "ars-nouveau"),
    ("铁魔法", "Iron's Spells 'n Spellbooks", "irons-spells-n-spellbooks"),
    ("神化", "Apotheosis", "apotheosis"),
    ("天境", "The Aether", "aether"),
    ("史诗战斗", "Epic Fight", "epic-fight"),
    ("更好的战斗", "Better Combat", "better-combat"),
    ("安全工艺", "SecurityCraft", "security-craft"),
    ("搬运", "Carry On", "carry-on"),
    ("自然指南针", "Nature's Compass", "natures-compass"),
    ("探险家指南针", "Explorer's Compass", "explorers-compass"),
    ("火把大师", "Torchmaster", "torchmaster"),
    ("连锁挖矿", "FTB Ultimine", "ftb-ultimine"),
    ("FTB任务", "FTB Quests", "ftb-quests"),
    ("地毯", "Carpet", "carpet"),
    ("创世神", "WorldEdit", "worldedit"),
    ("幸运方块", "Lucky Block", "lucky-block"),
    ("帕秋莉手册", "Patchouli", "patchouli"),
    ("饰品API", "Curios API", "curios"),
    ("饰品", "Baubles", "baubles"),
    ("遥远的地平线", "Distant Horizons", "distanthorizons"),
    ("瓦尔基里天空", "Valkyrien Skies", "valkyrien-skies"),
    ("沉浸传送门", "Immersive Portals", "immersive-portals"),
    ("静谧四季", "Serene Seasons", "serene-seasons"),
    ("意志坚定", "Tough As Nails", "tough-as-nails"),
    ("魔改器", "CraftTweaker", "crafttweaker"),
    ("植物盆", "Botany Pots", "botany-pots"),
    ("第一人称模型", "First-person Model", "first-person-model"),
    ("3D皮肤层", "3D Skin Layers", "3dskinlayers"),
    ("万用皮肤补丁", "CustomSkinLoader", "customskinloader"),
    ("自动汉化更新", "I18nUpdateMod", "i18nupdatemod"),
    ("模拟殖民地", "MineColonies", "minecolonies"),
    ("集成动力", "Integrated Dynamics", "integrated-dynamics"),
    ("铁路", "Railcraft", "railcraft"),
    ("水产养殖2|水产养殖", "Aquaculture 2", "aquaculture"),
    ("群峦传说", "TerraFirmaCraft", "terrafirmacraft"),
    ("吸血鬼", "Vampirism", "vampirism"),
    ("等价交换", "ProjectE", "projecte"),
    ("无尽贪婪", "Avaritia", "avaritia"),
    ("扩展合成", "Extended Crafting", "extended-crafting"),
    ("实用拓展", "Actually Additions", "actually-additions"),
    ("随机东西", "Random Things", "random-things"),
    ("更多实用设备2|更多实用设备", "Extra Utilities 2", "extra-utilities-2"),
    ("建筑", "BuildCraft", "buildcraft"),
    ("永恒枪械工坊", "Timeless and Classics Zero", "timeless-and-classics-zero"),
    ("高级火箭", "Advanced Rocketry", "advanced-rocketry"),
];

pub fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'))
}

/// 比名字用：只留字母、数字和汉字，小写
pub fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// 中文名和搜的词有多像：4 一样、3 开头是、2 包含、1 搜的词里包含这个名字
fn score(q: &str, zh: &str) -> Option<u8> {
    if q.is_empty() || zh.is_empty() {
        return None;
    }
    if q == zh {
        Some(4)
    } else if zh.starts_with(q) {
        Some(3)
    } else if zh.contains(q) {
        Some(2)
    } else if zh.chars().count() >= 2 && q.contains(zh) {
        Some(1)
    } else {
        None
    }
}

fn from_dict(q: &str) -> Vec<(u8, Name)> {
    DICT.iter()
        .filter_map(|(zhs, en, slug)| {
            let s = zhs.split('|').filter_map(|z| score(q, &norm(z))).max()?;
            Some((s, Name { zh: zhs.split('|').next().unwrap_or("").into(), en: (*en).into(), slug: (*slug).into() }))
        })
        .collect()
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn decode(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&quot;", "\"").replace("&#39;", "'").replace("&#039;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// 「[AS] 苹果皮 (AppleSkin)」→ 苹果皮 / AppleSkin。英文名自己也可能带括号，从后往前配对
fn split_title(t: &str) -> Option<Name> {
    let mut t = t.trim();
    if t.starts_with('[') {
        if let Some(i) = t.find(']') {
            t = t[i + 1..].trim();
        }
    }
    if t.ends_with(')') {
        let mut depth = 0;
        let mut open = None;
        for (i, c) in t.char_indices().rev() {
            match c {
                ')' => depth += 1,
                '(' => {
                    depth -= 1;
                    if depth == 0 {
                        open = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(o) = open {
            let en = t[o + 1..t.len() - 1].trim();
            let zh = t[..o].trim();
            if !en.is_empty() && !has_cjk(en) {
                return Some(Name { zh: zh.into(), en: en.into(), slug: String::new() });
            }
        }
    }
    // 没有中文名的：标题就是英文名
    (!t.is_empty() && !has_cjk(t)).then(|| Name { zh: String::new(), en: t.into(), slug: String::new() })
}

/// MC百科搜索页：每个结果的 head 里，指向 /class/数字.html 的那个链接就是标题
fn parse_mcmod(html: &str) -> Vec<Name> {
    let mut out = vec![];
    for part in html.split("class=\"head\"").skip(1) {
        let seg = part.split("class=\"body\"").next().unwrap_or(part);
        let mut at = 0;
        while let Some(i) = seg[at..].find("www.mcmod.cn/class/") {
            let start = at + i;
            at = start + 1;
            let rest = &seg[start + "www.mcmod.cn/class/".len()..];
            if !rest.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            let Some(gt) = rest.find('>') else { break };
            let Some(close) = rest[gt..].find("</a>") else { break };
            if let Some(n) = split_title(&decode(&strip_tags(&rest[gt + 1..gt + close]))) {
                out.push(n);
            }
            break;
        }
    }
    out
}

type Cache = HashMap<String, (Instant, Vec<Name>)>;
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

async fn mcmod(http: &reqwest::Client, q: &str) -> Result<Vec<Name>> {
    if let Some((at, v)) = CACHE.lock().unwrap().get_or_insert_with(HashMap::new).get(q) {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok(v.clone());
        }
    }
    let url = format!("https://search.mcmod.cn/s?key={}&filter=1&mold=1", enc(q));
    let r = http.get(&url).timeout(Duration::from_secs(8)).send().await.context("连不上 MC百科")?;
    if !r.status().is_success() {
        bail!("MC百科返回 {}", r.status());
    }
    let v = parse_mcmod(&r.text().await?);
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(q.to_string(), (Instant::now(), v.clone()));
    Ok(v)
}

/// 搜的词可能指哪些 Mod（最像的在前，最多 6 个）。MC百科连不上就只用自带的
pub async fn candidates(http: &reqwest::Client, q: &str) -> Vec<Name> {
    let q = q.trim();
    let qn = norm(q);
    let mut list: Vec<(u8, bool, Name)> = from_dict(&qn).into_iter().map(|(s, n)| (s, true, n)).collect();
    if let Ok(v) = mcmod(http, q).await {
        for n in v {
            let s = score(&qn, &norm(&n.zh)).unwrap_or(0);
            list.push((s, false, n));
        }
    }
    // 分数高的在前；一样高时自带的在前，其余保持 MC百科的顺序
    list.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut seen = HashSet::new();
    list.into_iter().filter(|(_, _, n)| seen.insert(norm(&n.en))).map(|x| x.2).take(6).collect()
}

/// 搜出来的这一项是不是这个名字指的 Mod。标题常带括号后缀：「Just Enough Items (JEI)」。
/// 冒号后面的不去掉：「Create: Food」这种是附属，不是机械动力本体
pub fn matches(n: &Name, title: &str, slug: &str) -> bool {
    if !n.slug.is_empty() && n.slug.eq_ignore_ascii_case(slug) {
        return true;
    }
    let en = norm(&n.en);
    if en.is_empty() {
        return false;
    }
    let base = title.split(['(', '[']).next().unwrap_or(title);
    norm(title) == en || norm(base) == en || norm(slug) == en
}

/// 英文搜出来的结果，认得的也标上中文名
pub fn annotate(hits: &mut [Hit]) {
    for h in hits.iter_mut().filter(|h| h.zh.is_none()) {
        h.zh = DICT
            .iter()
            .find(|(_, en, slug)| matches(&Name { zh: String::new(), en: (*en).into(), slug: (*slug).into() }, &h.title, &h.slug))
            .map(|(zhs, _, _)| zhs.split('|').next().unwrap_or("").to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles() {
        let n = split_title("[AS] 苹果皮 (AppleSkin)").unwrap();
        assert_eq!((n.zh.as_str(), n.en.as_str()), ("苹果皮", "AppleSkin"));
        let n = split_title("机械动力：咖啡馆 (Fabric) (Create Cafe Fabric)").unwrap();
        assert_eq!((n.zh.as_str(), n.en.as_str()), ("机械动力：咖啡馆 (Fabric)", "Create Cafe Fabric"));
        let n = split_title("苹果数据（数字版苹果皮） (AppleStats)").unwrap();
        assert_eq!(n.en, "AppleStats");
        assert_eq!(split_title("MCSR Sodium").unwrap().en, "MCSR Sodium");
        assert!(split_title("机械动力:更多过山车配置").is_none());
    }

    #[test]
    fn parse_page() {
        let html = r#"<div class="result-item"><div class="head"><div class="class-category"><ul><li><a class="c_24" href="//www.mcmod.cn/class/category/24-1.html" target="_blank"></a></li></ul></div><a  target="_blank" href="https://www.mcmod.cn/class/744.html">[AS] <em>苹果</em><em>皮</em> (AppleSkin)</a></div><div class="body">介绍 <a href="https://www.mcmod.cn/class/1.html">别的</a></div></div>"#;
        let v = parse_mcmod(html);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].en, "AppleSkin");
        assert_eq!(v[0].zh, "苹果皮");
    }

    #[test]
    fn dict_and_match() {
        let d = from_dict(&norm("机械动力"));
        assert!(d.iter().any(|(s, n)| *s == 4 && n.en == "Create"));
        let jei = Name { zh: "物品管理器".into(), en: "Just Enough Items".into(), slug: String::new() };
        assert!(matches(&jei, "Just Enough Items (JEI)", "jei"));
        assert!(!matches(&jei, "Just Enough Resources (JER)", "jer"));
        let iaf = Name { zh: String::new(), en: "Ice and Fire".into(), slug: "ice-and-fire-dragons".into() };
        assert!(matches(&iaf, "Ice and Fire: Dragons", "ice-and-fire-dragons"));
        let create = Name { zh: "机械动力".into(), en: "Create".into(), slug: "create".into() };
        assert!(!matches(&create, "Create: Food", "create-food"));
        assert!(matches(&create, "Create", "create"));
    }
}
