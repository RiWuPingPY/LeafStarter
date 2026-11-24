//! 读 Mod 文件里的元数据：id、名字、版本、作者、必需的前置、链接（ROADMAP 5.6 本地 Mod、5.3 二分查冲突）。
//!
//! - Fabric：`fabric.mod.json`
//! - Quilt：`quilt.mod.json`
//! - Forge 1.13+：`META-INF/mods.toml`；NeoForge：`META-INF/neoforge.mods.toml`
//! - Forge 1.12 及更早：`mcmod.info`

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModMeta {
    pub file: String,
    pub enabled: bool,
    /// fabric / quilt / forge / neoforge / legacy-forge；认不出是空的
    pub loader: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    /// 必需的前置 Mod 的 id（不含游戏本体、加载器、Java）
    pub deps: Vec<String>,
    /// 这个文件还提供哪些 id（Fabric 的 provides、一个 jar 里多个 Mod）
    pub provides: Vec<String>,
    pub homepage: Option<String>,
    pub sources: Option<String>,
    pub issues: Option<String>,
}

const PLATFORM: &[&str] = &["minecraft", "java", "fabricloader", "fabric-loader", "quilt_loader", "quilt-loader", "forge", "neoforge", "fml", "javafml", "mcp", "lowcodefml"];

fn platform(id: &str) -> bool {
    PLATFORM.contains(&id.to_ascii_lowercase().as_str())
}

fn read_entry<R: Read + std::io::Seek>(z: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let mut f = z.by_name(name).ok()?;
    if f.size() > 4 * 1024 * 1024 {
        return None;
    }
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s.trim_start_matches('\u{feff}').to_string())
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map(String::from)
}

/// 读一个 Mod 文件；不是 zip 或者认不出也返回一个只有文件名的。
/// `prefer` 是实例的加载器（Forge / NeoForge / Fabric / Quilt）：同一个 jar 里几家的元数据都有时，按它挑
pub fn read(path: &Path, prefer: &str) -> ModMeta {
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let enabled = !file.to_ascii_lowercase().ends_with(".disabled");
    let mut m = ModMeta { file, enabled, ..Default::default() };
    let Ok(f) = std::fs::File::open(path) else { return m };
    let Ok(mut z) = zip::ZipArchive::new(std::io::BufReader::new(f)) else { return m };
    parse(&mut z, &mut m, prefer);
    // 打包在里面的 Mod（Forge 的 jarjar、Fabric 的 META-INF/jars）：它们的 id 也算这个文件提供的
    let mut nested = vec![];
    nested_ids(&mut z, 0, &mut nested);
    for id in nested {
        if Some(&id) != m.id.as_ref() && !m.provides.contains(&id) {
            m.provides.push(id);
        }
    }
    m
}

fn nested_ids<R: Read + std::io::Seek>(z: &mut zip::ZipArchive<R>, depth: u32, out: &mut Vec<String>) {
    if depth > 2 {
        return;
    }
    let names: Vec<String> = z.file_names().filter(|n| (n.starts_with("META-INF/jarjar/") || n.starts_with("META-INF/jars/")) && n.ends_with(".jar")).map(String::from).collect();
    for n in names {
        let Ok(mut f) = z.by_name(&n) else { continue };
        if f.size() > 64 * 1024 * 1024 {
            continue;
        }
        let mut buf = Vec::with_capacity(f.size() as usize);
        if f.read_to_end(&mut buf).is_err() {
            continue;
        }
        drop(f);
        let Ok(mut inner) = zip::ZipArchive::new(std::io::Cursor::new(buf)) else { continue };
        let mut m = ModMeta::default();
        parse(&mut inner, &mut m, "");
        out.extend(m.id);
        out.extend(m.provides);
        nested_ids(&mut inner, depth + 1, out);
    }
}

fn parse<R: Read + std::io::Seek>(z: &mut zip::ZipArchive<R>, m: &mut ModMeta, prefer: &str) {
    let p = prefer.to_ascii_lowercase();
    let order: [&str; 5] = match p.as_str() {
        "forge" => ["forge", "neoforge", "fabric", "quilt", "mcmod"],
        "neoforge" => ["neoforge", "forge", "fabric", "quilt", "mcmod"],
        "quilt" => ["quilt", "fabric", "neoforge", "forge", "mcmod"],
        _ => ["fabric", "quilt", "neoforge", "forge", "mcmod"],
    };
    for kind in order {
        let ok = match kind {
            "fabric" => read_entry(z, "fabric.mod.json").and_then(|t| serde_json::from_str::<Value>(&lenient_json(&t)).ok()).map(|v| fabric(m, &v)).is_some(),
            "quilt" => read_entry(z, "quilt.mod.json").and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|v| quilt(m, &v)).is_some(),
            "forge" | "neoforge" => {
                let entry = if kind == "forge" { "META-INF/mods.toml" } else { "META-INF/neoforge.mods.toml" };
                match read_entry(z, entry) {
                    Some(t) => {
                        let manifest = read_entry(z, "META-INF/MANIFEST.MF").unwrap_or_default();
                        let ok = forge_toml(m, &t, &manifest);
                        if ok {
                            m.loader = kind.into();
                        }
                        ok
                    }
                    None => false,
                }
            }
            _ => read_entry(z, "mcmod.info").map(|t| mcmod(m, &t)).is_some() && m.id.is_some(),
        };
        if ok {
            return;
        }
    }
}

/// 有的 fabric.mod.json 里有换行写在字符串里之类的小毛病，Fabric 能读，serde 不能：把字符串里的换行转义掉
fn lenient_json(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut in_str = false;
    let mut esc = false;
    for c in t.chars() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            } else if c == '\n' {
                out.push_str("\\n");
                continue;
            } else if c == '\r' || c == '\t' {
                out.push(' ');
                continue;
            }
        } else if c == '"' {
            in_str = true;
        }
        out.push(c);
    }
    out
}

fn people(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from).or_else(|| s(x, "name"))).collect())
        .unwrap_or_default()
}

fn fabric(m: &mut ModMeta, v: &Value) {
    m.loader = "fabric".into();
    m.id = s(v, "id");
    m.name = s(v, "name");
    m.version = s(v, "version");
    m.description = s(v, "description");
    m.authors = people(v.get("authors"));
    if let Some(d) = v.get("depends").and_then(Value::as_object) {
        m.deps = d.keys().filter(|k| !platform(k)).cloned().collect();
    }
    m.provides = v.get("provides").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    if let Some(c) = v.get("contact") {
        m.homepage = s(c, "homepage");
        m.sources = s(c, "sources");
        m.issues = s(c, "issues");
    }
}

fn quilt(m: &mut ModMeta, v: &Value) {
    m.loader = "quilt".into();
    let q = v.get("quilt_loader").cloned().unwrap_or(Value::Null);
    m.id = s(&q, "id");
    m.version = s(&q, "version");
    let meta = q.get("metadata").cloned().unwrap_or(Value::Null);
    m.name = s(&meta, "name");
    m.description = s(&meta, "description");
    if let Some(c) = meta.get("contributors").and_then(Value::as_object) {
        m.authors = c.keys().cloned().collect();
    }
    if let Some(c) = meta.get("contact") {
        m.homepage = s(c, "homepage");
        m.sources = s(c, "sources");
        m.issues = s(c, "issues");
    }
    m.deps = q
        .get("depends")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|d| !d.get("optional").and_then(Value::as_bool).unwrap_or(false))
                .filter_map(|d| d.as_str().map(String::from).or_else(|| s(d, "id")))
                .filter(|id| !platform(id))
                .collect()
        })
        .unwrap_or_default();
    m.provides = q.get("provides").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from).or_else(|| s(x, "id"))).collect()).unwrap_or_default();
}

fn forge_toml(m: &mut ModMeta, text: &str, manifest: &str) -> bool {
    let Ok(t) = text.parse::<toml::Table>() else { return false };
    let mods = t.get("mods").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let Some(first) = mods.first() else { return false };
    let get = |x: &toml::Value, k: &str| x.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty()).map(String::from);
    m.id = get(first, "modId");
    m.name = get(first, "displayName");
    m.description = get(first, "description");
    m.version = get(first, "version").map(|v| {
        if v.contains("${file.jarVersion}") {
            manifest.lines().find_map(|l| l.strip_prefix("Implementation-Version:")).map(|x| x.trim().to_string()).unwrap_or(v)
        } else {
            v
        }
    });
    m.authors = get(first, "authors").map(|a| a.split([',', '&']).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    m.homepage = get(first, "displayURL");
    m.issues = t.get("issueTrackerURL").and_then(|v| v.as_str()).map(String::from);
    m.provides = mods.iter().skip(1).filter_map(|x| get(x, "modId")).collect();
    // [[dependencies.<modId>]]：老写法 mandatory=true，新写法 type="required"
    let mut deps = vec![];
    if let Some(all) = t.get("dependencies").and_then(|d| d.as_table()) {
        for list in all.values() {
            for d in list.as_array().into_iter().flatten() {
                let required = d.get("mandatory").and_then(|x| x.as_bool()).unwrap_or(false) || d.get("type").and_then(|x| x.as_str()).is_some_and(|x| x.eq_ignore_ascii_case("required"));
                let side_ok = d.get("side").and_then(|x| x.as_str()).is_none_or(|x| !x.eq_ignore_ascii_case("SERVER"));
                if let Some(id) = get(d, "modId") {
                    if required && side_ok && !platform(&id) && !deps.contains(&id) {
                        deps.push(id);
                    }
                }
            }
        }
    }
    m.deps = deps;
    m.id.is_some()
}

fn mcmod(m: &mut ModMeta, text: &str) {
    let Ok(v) = serde_json::from_str::<Value>(&lenient_json(text)) else { return };
    // 两种写法：直接是数组，或者 { modList: [...] }
    let list = v.as_array().cloned().or_else(|| v.get("modList").and_then(Value::as_array).cloned()).unwrap_or_default();
    let Some(first) = list.first() else { return };
    m.loader = "legacy-forge".into();
    m.id = s(first, "modid");
    m.name = s(first, "name");
    m.version = s(first, "version");
    m.description = s(first, "description");
    m.authors = people(first.get("authorList").or_else(|| first.get("authors")));
    m.homepage = s(first, "url");
    m.deps = first
        .get("requiredMods")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|x| x.split('@').next().unwrap_or(x).trim().to_string()).filter(|x| !x.is_empty() && !platform(x)).collect())
        .unwrap_or_default();
    m.provides = list.iter().skip(1).filter_map(|x| s(x, "modid")).collect();
}

/// 读一个 mods 文件夹（启用的、禁用的都读）
pub fn read_dir(mods: &Path, prefer: &str) -> Vec<ModMeta> {
    let mut out: Vec<ModMeta> = std::fs::read_dir(mods)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            e.path().is_file() && (n.ends_with(".jar") || n.ends_with(".jar.disabled") || n.ends_with(".zip") || n.ends_with(".zip.disabled"))
        })
        .map(|e| read(&e.path(), prefer))
        .collect();
    out.sort_by(|a, b| a.file.to_lowercase().cmp(&b.file.to_lowercase()));
    out
}

/// id 比较时 `-` 和 `_` 当一样、不分大小写（Fabric 写 fabric-api，Forge 移植版写 fabric_api）
fn norm_id(id: &str) -> String {
    id.to_ascii_lowercase().replace('-', "_")
}

/// 一组文件连同它们必需的前置（递归），用于二分时「只开这些」。前置找不到就算了，交给游戏报错
pub fn closure(all: &[ModMeta], pick: &[usize]) -> Vec<usize> {
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for (k, m) in all.iter().enumerate() {
        for id in m.id.iter().chain(m.provides.iter()) {
            by_id.entry(norm_id(id)).or_insert(k);
        }
    }
    let mut seen: HashSet<usize> = HashSet::new();
    let mut stack: Vec<usize> = pick.to_vec();
    while let Some(k) = stack.pop() {
        if !seen.insert(k) {
            continue;
        }
        for d in &all[k].deps {
            if let Some(&j) = by_id.get(&norm_id(d)) {
                stack.push(j);
            }
        }
    }
    let mut v: Vec<usize> = seen.into_iter().collect();
    v.sort();
    v
}

/// 缺了哪些必需的前置：(Mod 文件, 缺的 id)
/// `prefer` 是实例的加载器：Forge 整合包里靠 Connector 跑的 Fabric Mod，前置 id 会被换掉，不查它们
pub fn missing_deps(all: &[ModMeta], prefer: &str) -> Vec<(String, String)> {
    let family = |l: &str| match l.to_ascii_lowercase().as_str() {
        "fabric" | "quilt" => 1,
        "forge" | "neoforge" | "legacy-forge" => 2,
        _ => 0,
    };
    let want = family(prefer);
    let have: HashSet<String> = all.iter().filter(|m| m.enabled).flat_map(|m| m.id.iter().chain(m.provides.iter()).map(|x| norm_id(x))).collect();
    let mut out = vec![];
    for m in all.iter().filter(|m| m.enabled && (want == 0 || family(&m.loader) == 0 || family(&m.loader) == want)) {
        for d in &m.deps {
            let d2 = norm_id(d);
            // Fabric API 的模块常写成 fabric-xxx-api-v1，都在 fabric-api 这一个文件里
            if !have.contains(&d2) && !(d2.starts_with("fabric_") && have.contains("fabric_api")) && d2 != "fabric" {
                out.push((m.file.clone(), d.clone()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(dir: &Path, name: &str, files: &[(&str, &str)]) {
        let f = std::fs::File::create(dir.join(name)).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (n, t) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(t.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn reads_all_loaders() {
        let d = std::env::temp_dir().join(format!("ls-mods-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        jar(&d, "sodium.jar", &[("fabric.mod.json", "{\"id\":\"sodium\",\"name\":\"Sodium\",\"version\":\"0.5.8\",\"description\":\"line1\nline2\",\"authors\":[\"jellysquid\",{\"name\":\"IMS\"}],\"depends\":{\"minecraft\":\"1.20.1\",\"fabric-api\":\"*\"},\"contact\":{\"sources\":\"https://github.com/x/sodium\"}}")]);
        jar(&d, "jei.jar.disabled", &[
            ("META-INF/mods.toml", "modLoader=\"javafml\"\nissueTrackerURL=\"https://github.com/mezz/JEI/issues\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"Just Enough Items\"\nauthors=\"mezz\"\n[[dependencies.jei]]\nmodId=\"forge\"\nmandatory=true\n[[dependencies.jei]]\nmodId=\"architectury\"\ntype=\"required\"\n[[dependencies.jei]]\nmodId=\"optional_thing\"\nmandatory=false\n"),
            ("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\nImplementation-Version: 15.2.0.27\n"),
        ]);
        jar(&d, "old.jar", &[("mcmod.info", "[{\"modid\":\"ic2\",\"name\":\"IndustrialCraft 2\",\"version\":\"2.8\",\"requiredMods\":[\"Forge@[10.13,)\",\"buildcraft\"]}]")]);
        jar(&d, "nope.jar", &[("a.txt", "x")]);
        let all = read_dir(&d, "");
        let by = |f: &str| all.iter().find(|m| m.file == f).unwrap();
        let s = by("sodium.jar");
        assert_eq!((s.loader.as_str(), s.id.as_deref(), s.deps.clone()), ("fabric", Some("sodium"), vec!["fabric-api".to_string()]));
        assert_eq!(s.authors, vec!["jellysquid", "IMS"]);
        let j = by("jei.jar.disabled");
        assert_eq!((j.enabled, j.version.as_deref(), j.deps.clone()), (false, Some("15.2.0.27"), vec!["architectury".to_string()]));
        assert_eq!(by("old.jar").deps, vec!["buildcraft".to_string()]);
        assert_eq!(by("nope.jar").id, None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn closure_follows_deps() {
        let m = |id: &str, deps: &[&str]| ModMeta { id: Some(id.into()), deps: deps.iter().map(|x| x.to_string()).collect(), enabled: true, ..Default::default() };
        let all = vec![m("a", &["lib"]), m("lib", &["core"]), m("core", &[]), m("b", &["missing"])];
        assert_eq!(closure(&all, &[0]), vec![0, 1, 2]);
        assert_eq!(closure(&all, &[3]), vec![3]);
        assert_eq!(missing_deps(&all, ""), vec![(String::new(), "missing".to_string())]);
    }
}
