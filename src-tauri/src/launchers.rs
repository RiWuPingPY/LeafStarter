//! 识别本机装了哪些启动器、它们的 .minecraft 在哪。
//!
//! Windows：官方目录 + PCL 注册表 + 在常用位置有限深度地找启动器程序和 .minecraft 文件夹。
//! 安卓：按包名认启动器；公共存储里的目录直接写，Android/data 里的目录走启动器自己提供的
//! DocumentsProvider（用户在系统文件选择器里授权一次）。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::mojang::{installed_versions, InstalledVersion};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub id: String,
    /// pcl / pcl-ce / hmcl / official / bakaxl / sjmcl / zl2 / zl / fcl / pojav / hmclpe / custom
    pub kind: String,
    pub launcher: String,
    pub path: String,
    /// direct：可以直接写；saf：已授权，通过系统文件接口写；need-saf：需要去授权；need-permission：需要"所有文件访问"权限
    pub access: String,
    pub exe: Option<String>,
    pub versions: Vec<InstalledVersion>,
    pub manual: bool,
    pub saf_uri: Option<String>,
    pub saf_rel: Option<String>,
    pub package: Option<String>,
    pub hint: Option<String>,
}

/// 安卓上用户授权过的目录树
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafGrant {
    pub uri: String,
    /// .minecraft 在这棵树里的相对路径（空字符串表示树根就是 .minecraft）
    pub rel: String,
    pub package: Option<String>,
    pub label: String,
}

pub fn target_id(s: &str) -> String {
    // FNV-1a，够用来做稳定的短 id
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.to_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:012x}", h & 0xffff_ffff_ffff)
}

pub fn launcher_name(kind: &str) -> &'static str {
    match kind {
        "pcl" => "PCL2",
        "pcl-ce" => "PCL 社区版",
        "hmcl" => "HMCL",
        "official" => "官方启动器",
        "bakaxl" => "BakaXL",
        "sjmcl" => "SJMCL",
        "zl2" => "Zalith Launcher 2",
        "zl" => "Zalith Launcher",
        "fcl" => "Fold Craft Launcher",
        "pojav" => "PojavLauncher",
        "hmclpe" => "HMCL-PE",
        _ => "自定义目录",
    }
}

fn kind_rank(kind: &str) -> u8 {
    if kind == "custom" {
        0
    } else {
        1
    }
}

struct Found {
    mc: PathBuf,
    kind: String,
    exe: Option<PathBuf>,
}

/// 根据目录里的痕迹判断归属
fn classify(mc: &Path, hint: &str) -> String {
    if hint != "custom" {
        return hint.to_string();
    }
    let parent = mc.parent();
    if mc.join("PCL.ini").is_file() || parent.map_or(false, |p| p.join("PCL").is_dir()) {
        return "pcl".into();
    }
    if parent.map_or(false, |p| p.join("hmcl.json").is_file() || p.join(".hmcl").is_dir()) {
        return "hmcl".into();
    }
    if mc.join("BakaXL").is_dir() || parent.map_or(false, |p| p.join("BakaXL").is_dir()) {
        return "bakaxl".into();
    }
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        if norm(mc) == norm(&Path::new(&appdata).join(".minecraft")) {
            return "official".into();
        }
    }
    "custom".into()
}

fn norm(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/');
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// 启动器程序文件名 → 类型
fn exe_kind(file_name: &str) -> Option<&'static str> {
    let n = file_name.to_lowercase();
    let is_prog = n.ends_with(".exe") || n.ends_with(".jar");
    if !is_prog {
        return None;
    }
    if n.starts_with("plain craft launcher") {
        return Some(if n.contains("community") || n.contains(" ce") { "pcl-ce" } else { "pcl" });
    }
    if n.starts_with("pcl") && n.ends_with(".exe") {
        let rest = &n[3..];
        if rest.starts_with(".exe") || rest.starts_with('2') || rest.starts_with(' ') || rest.starts_with('-') || rest.starts_with('_') {
            return Some(if n.contains("ce") || n.contains("community") { "pcl-ce" } else { "pcl" });
        }
    }
    if n.starts_with("hmcl") {
        return Some("hmcl");
    }
    if n.starts_with("bakaxl") {
        return Some("bakaxl");
    }
    if n.starts_with("sjmcl") {
        return Some("sjmcl");
    }
    None
}

/// HMCL 的 hmcl.json 里每个配置都有 gameDir
fn hmcl_game_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for cfg in [dir.join("hmcl.json"), dir.join(".hmcl").join("hmcl.json")] {
        let Ok(text) = std::fs::read_to_string(&cfg) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        for (_, prof) in v.get("configurations").and_then(|c| c.as_object()).into_iter().flatten() {
            if let Some(g) = prof.get("gameDir").and_then(|g| g.as_str()) {
                let p = Path::new(g);
                out.push(if p.is_absolute() { p.to_path_buf() } else { dir.join(p) });
            }
        }
    }
    out
}

/// 有限深度、有限时间的目录遍历
fn walk_for_launchers(roots: &[(PathBuf, u32)], deadline: Instant, found: &mut Vec<Found>) {
    const SKIP: &[&str] = &[
        "windows", "windows.old", "$recycle.bin", "system volume information", "recovery", "perflogs", "programdata",
        "node_modules", ".git", "appdata", "program files", "program files (x86)", "msys64", "$windows.~bt", "$winreagent",
        "assets", "libraries", "saves", "mods", "resourcepacks", "shaderpacks", "steamapps", "target", ".gradle", ".cargo",
        ".rustup", ".npm", "site-packages",
    ];
    let mut queue: VecDeque<(PathBuf, u32)> = roots.iter().cloned().collect();
    let mut visited = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if Instant::now() > deadline || visited > 60_000 {
            break;
        }
        visited += 1;
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                let lower = name.to_lowercase();
                if lower == ".minecraft" {
                    if e.path().join("versions").is_dir() {
                        found.push(Found { mc: e.path(), kind: "custom".into(), exe: None });
                    }
                    continue;
                }
                if depth > 0 && !SKIP.contains(&lower.as_str()) && !lower.starts_with('$') {
                    queue.push_back((e.path(), depth - 1));
                }
            } else if let Some(kind) = exe_kind(&name) {
                let exe = e.path();
                let mut dirs = if kind == "hmcl" { hmcl_game_dirs(&dir) } else { vec![] };
                dirs.push(dir.join(".minecraft"));
                for mc in dirs {
                    found.push(Found { mc, kind: kind.into(), exe: Some(exe.clone()) });
                }
            }
        }
    }
}

#[cfg(windows)]
fn pcl_registry(found: &mut Vec<Found>) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    for (sub, kind) in [("Software\\PCL", "pcl"), ("Software\\PCLCE", "pcl-ce"), ("Software\\PCL-CE", "pcl-ce")] {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(sub) else { continue };
        // 格式：名称>路径|名称>路径|
        if let Ok(folders) = key.get_value::<String, _>("LaunchFolders") {
            for item in folders.split('|').filter(|s| !s.is_empty()) {
                let path = item.split_once('>').map(|(_, p)| p).unwrap_or(item).trim();
                if !path.is_empty() && !path.starts_with('$') {
                    found.push(Found { mc: PathBuf::from(path), kind: kind.into(), exe: None });
                }
            }
        }
        // 下载缓存目录一般是 <PCL 所在目录>\PCL\MyDownload\，可以反推出 PCL 在哪
        if let Ok(dl) = key.get_value::<String, _>("CacheDownloadFolder") {
            let p = PathBuf::from(dl.trim_end_matches('\\'));
            if p.file_name().map_or(false, |n| n.eq_ignore_ascii_case("MyDownload")) {
                if let Some(pcl_dir) = p.parent().and_then(|x| x.parent()) {
                    let exe = std::fs::read_dir(pcl_dir)
                        .ok()
                        .and_then(|rd| rd.flatten().find(|e| exe_kind(&e.file_name().to_string_lossy()).is_some()))
                        .map(|e| e.path());
                    found.push(Found { mc: pcl_dir.join(".minecraft"), kind: kind.into(), exe });
                }
            }
        }
    }
}

fn to_target(f: Found, manual: bool) -> Target {
    let kind = classify(&f.mc, &f.kind);
    let path = f.mc.to_string_lossy().into_owned();
    Target {
        id: target_id(&path),
        launcher: launcher_name(&kind).to_string(),
        kind,
        versions: installed_versions(&f.mc),
        path,
        access: "direct".into(),
        exe: f.exe.map(|e| e.to_string_lossy().into_owned()),
        manual,
        saf_uri: None,
        saf_rel: None,
        package: None,
        hint: None,
    }
}

fn merge_found(found: Vec<Found>, manual: &[String]) -> Vec<Target> {
    let mut map: HashMap<String, Target> = HashMap::new();
    let mut order: Vec<String> = vec![];
    let manual_found = manual.iter().map(|m| {
        let p = PathBuf::from(m);
        let mc = if p.file_name().map_or(false, |n| n == ".minecraft") || p.join("versions").is_dir() || !p.join(".minecraft").is_dir() {
            p
        } else {
            p.join(".minecraft")
        };
        (Found { mc, kind: "custom".into(), exe: None }, true)
    });
    for (f, is_manual) in found.into_iter().map(|f| (f, false)).chain(manual_found) {
        // 刚下好还没启动过的启动器没有 .minecraft，但找到了程序文件就保留（安装时会建目录）
        if !is_manual && !f.mc.is_dir() && f.exe.is_none() {
            continue;
        }
        let key = norm(&f.mc);
        let t = to_target(f, is_manual);
        match map.get_mut(&key) {
            Some(existing) => {
                if kind_rank(&t.kind) > kind_rank(&existing.kind) {
                    existing.kind = t.kind.clone();
                    existing.launcher = t.launcher.clone();
                }
                if existing.exe.is_none() {
                    existing.exe = t.exe;
                }
                existing.manual |= t.manual;
            }
            None => {
                order.push(key.clone());
                map.insert(key, t);
            }
        }
    }
    let mut out: Vec<Target> = order.into_iter().filter_map(|k| map.remove(&k)).collect();
    // 有程序文件的、版本多的排前面
    out.sort_by(|a, b| {
        kind_rank(&b.kind)
            .cmp(&kind_rank(&a.kind))
            .then(b.exe.is_some().cmp(&a.exe.is_some()))
            .then(b.versions.len().cmp(&a.versions.len()))
    });
    out
}

#[cfg(windows)]
pub fn scan_desktop(manual: &[String]) -> Vec<Target> {
    let mut found = vec![];
    if let Ok(appdata) = std::env::var("APPDATA") {
        let mc = Path::new(&appdata).join(".minecraft");
        if mc.is_dir() {
            found.push(Found { mc, kind: "official".into(), exe: None });
        }
    }
    pcl_registry(&mut found);

    let mut roots: Vec<(PathBuf, u32)> = vec![];
    if let Some(home) = dirs::home_dir() {
        for sub in ["Desktop", "Downloads", "Documents", "桌面", "下载", "文档"] {
            roots.push((home.join(sub), 3));
        }
        roots.push((home.clone(), 1));
    }
    for letter in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", letter as char));
        if root.is_dir() {
            roots.push((root, 3));
        }
    }
    walk_for_launchers(&roots, Instant::now() + Duration::from_secs(6), &mut found);
    merge_found(found, manual)
}

#[cfg(not(any(windows, target_os = "android")))]
pub fn scan_desktop(manual: &[String]) -> Vec<Target> {
    let mut found = vec![];
    if let Some(home) = dirs::home_dir() {
        for mc in [home.join(".minecraft"), home.join("Library/Application Support/minecraft")] {
            if mc.is_dir() {
                found.push(Found { mc, kind: "official".into(), exe: None });
            }
        }
        walk_for_launchers(&[(home, 3)], Instant::now() + Duration::from_secs(6), &mut found);
    }
    merge_found(found, manual)
}

/// 安卓上已知的启动器：包名、类型、公共存储里的默认目录
pub const ANDROID_LAUNCHERS: &[(&str, &str, &[&str])] = &[
    ("com.movtery.zalithlauncher.v2", "zl2", &[]),
    ("com.movtery.zalithlauncher", "zl", &[]),
    ("com.tungsten.fcl", "fcl", &["FCL/.minecraft"]),
    ("net.kdt.pojavlaunch", "pojav", &["games/PojavLauncher/.minecraft"]),
    ("net.kdt.pojavlaunch.debug", "pojav", &[]),
    ("com.tungsten.hmclpe", "hmclpe", &["HMCLPE/.minecraft", "games/HMCLPE/.minecraft"]),
];

pub const ANDROID_STORAGE: &str = "/storage/emulated/0";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub package: String,
    pub label: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub version: String,
}

/// 安卓扫描。`apps` 是已安装的已知启动器，`all_files` 表示是否有"所有文件访问"权限，
/// `saf_versions` 是已授权目录里的版本列表（由 Kotlin 那边读出来）。
pub fn scan_android(
    apps: &[InstalledApp],
    all_files: bool,
    grants: &[SafGrant],
    saf_versions: &HashMap<String, Vec<InstalledVersion>>,
    manual: &[String],
) -> Vec<Target> {
    let storage = Path::new(ANDROID_STORAGE);
    let mut out: Vec<Target> = vec![];
    let mut seen_paths: Vec<String> = vec![];

    for app in apps {
        let Some((_, kind, public)) = ANDROID_LAUNCHERS.iter().find(|(p, _, _)| *p == app.package) else { continue };
        let name = if app.label.is_empty() { launcher_name(kind).to_string() } else { app.label.clone() };

        // 公共存储里的目录：有权限就直接写
        for rel in public.iter() {
            let mc = storage.join(rel);
            let exists = all_files && mc.is_dir();
            if !all_files || exists {
                let path = mc.to_string_lossy().into_owned();
                seen_paths.push(norm(&mc));
                out.push(Target {
                    id: target_id(&path),
                    kind: kind.to_string(),
                    launcher: name.clone(),
                    versions: if exists { installed_versions(&mc) } else { vec![] },
                    path,
                    access: if all_files { "direct".into() } else { "need-permission".into() },
                    exe: None,
                    manual: false,
                    saf_uri: None,
                    saf_rel: None,
                    package: Some(app.package.clone()),
                    hint: Some("公共存储目录".into()),
                });
            }
        }

        // 私有目录 Android/data/<包名>/files/.minecraft：要通过启动器的文件提供器授权
        let private = format!("{}/Android/data/{}/files/.minecraft", ANDROID_STORAGE, app.package);
        let grant = grants.iter().find(|g| g.package.as_deref() == Some(app.package.as_str()));
        out.push(Target {
            id: target_id(&private),
            kind: kind.to_string(),
            launcher: name.clone(),
            versions: grant.and_then(|g| saf_versions.get(&g.uri)).cloned().unwrap_or_default(),
            path: private,
            access: if grant.is_some() { "saf".into() } else { "need-saf".into() },
            exe: None,
            manual: false,
            saf_uri: grant.map(|g| g.uri.clone()),
            saf_rel: grant.map(|g| g.rel.clone()),
            package: Some(app.package.clone()),
            hint: Some("启动器默认目录".into()),
        });
    }

    // 不属于已知启动器的授权目录（比如用户在 ZL2 里自己加的游戏目录）
    for g in grants.iter().filter(|g| g.package.is_none()) {
        out.push(Target {
            id: target_id(&g.uri),
            kind: "custom".into(),
            launcher: launcher_name("custom").into(),
            versions: saf_versions.get(&g.uri).cloned().unwrap_or_default(),
            path: g.label.clone(),
            access: "saf".into(),
            exe: None,
            manual: true,
            saf_uri: Some(g.uri.clone()),
            saf_rel: Some(g.rel.clone()),
            package: None,
            hint: None,
        });
    }

    // 有权限时在公共存储里再找一圈 .minecraft
    if all_files {
        let mut found = vec![];
        let roots = vec![(storage.to_path_buf(), 3)];
        walk_for_launchers(&roots, Instant::now() + Duration::from_secs(4), &mut found);
        for t in merge_found(found, manual) {
            let key = norm(Path::new(&t.path));
            if !seen_paths.contains(&key) {
                seen_paths.push(key);
                out.push(t);
            }
        }
    } else {
        out.extend(merge_found(vec![], manual));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_detection() {
        assert_eq!(exe_kind("Plain Craft Launcher 2.exe"), Some("pcl"));
        assert_eq!(exe_kind("Plain Craft Launcher Community Edition.exe"), Some("pcl-ce"));
        assert_eq!(exe_kind("PCL2.exe"), Some("pcl"));
        assert_eq!(exe_kind("HMCL-3.6.12.exe"), Some("hmcl"));
        assert_eq!(exe_kind("HMCL-3.6.12.jar"), Some("hmcl"));
        assert_eq!(exe_kind("pclient.exe"), None);
        assert_eq!(exe_kind("readme.txt"), None);
    }
}
