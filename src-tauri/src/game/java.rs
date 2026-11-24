//! Java：找本机已有的（包括各启动器下载的运行时），找不到就从 Mojang 拉一份官方运行时。
//! 只有安装 Forge / NeoForge 时要用到。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::base::net::{FileJob, Net, Progress};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    pub path: String,
    pub major: u32,
    pub version: String,
    pub source: String,
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

/// 从 JAVA_HOME/release 读版本号
fn read_release(home: &Path) -> Option<String> {
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("JAVA_VERSION="))
        .map(|v| v.trim().trim_matches('"').to_string())
}

pub fn major_of(version: &str) -> u32 {
    let mut parts = version.split(|c: char| c == '.' || c == '_' || c == '+' || c == '-');
    let first: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if first == 1 {
        parts.next().and_then(|p| p.parse().ok()).unwrap_or(0)
    } else {
        first
    }
}

fn probe(bin_java: &Path, source: &str) -> Option<JavaInfo> {
    if !bin_java.is_file() {
        return None;
    }
    let home = bin_java.parent()?.parent()?;
    let version = read_release(home).unwrap_or_default();
    Some(JavaInfo {
        path: bin_java.to_string_lossy().into_owned(),
        major: major_of(&version),
        version,
        source: source.to_string(),
    })
}

/// 在一个目录下找 bin/java(.exe)，最多往下 4 层（Mojang 运行时的结构比较深）
fn scan_dir(dir: &Path, depth: u32, source: &str, out: &mut Vec<JavaInfo>) {
    if depth == 0 {
        return;
    }
    let candidate = dir.join("bin").join(exe_name());
    if let Some(info) = probe(&candidate, source) {
        out.push(info);
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            scan_dir(&e.path(), depth - 1, source, out);
        }
    }
}

pub fn detect(data_dir: &Path) -> Vec<JavaInfo> {
    let mut found = vec![];
    // 我们自己下载的运行时
    scan_dir(&data_dir.join("java"), 4, "LeafStarter 下载", &mut found);

    if let Ok(home) = std::env::var("JAVA_HOME") {
        if let Some(i) = probe(&Path::new(&home).join("bin").join(exe_name()), "JAVA_HOME") {
            found.push(i);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if let Some(i) = probe(&dir.join(exe_name()), "PATH") {
                found.push(i);
            }
        }
    }

    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").map(PathBuf::from).ok();
        let local = std::env::var("LOCALAPPDATA").map(PathBuf::from).ok();
        if let Some(a) = &appdata {
            scan_dir(&a.join(".minecraft").join("runtime"), 5, "官方启动器", &mut found);
            scan_dir(&a.join(".hmcl").join("java"), 4, "HMCL", &mut found);
        }
        if let Some(l) = &local {
            scan_dir(
                &l.join("Packages").join("Microsoft.4297127D64EC6_8wekyb3d8bbwe").join("LocalCache").join("Local").join("runtime"),
                5,
                "官方启动器（商店版）",
                &mut found,
            );
        }
        for pf in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(p) = std::env::var(pf) {
                for vendor in ["Java", "Eclipse Adoptium", "Zulu", "Microsoft", "BellSoft", "Amazon Corretto", "Semeru"] {
                    scan_dir(&Path::new(&p).join(vendor), 2, vendor, &mut found);
                }
            }
        }
        // PCL 记下来的 Java 列表
        if let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey("Software\\PCL") {
            if let Ok(json) = key.get_value::<String, _>("LaunchArgumentJavaAll") {
                if let Ok(list) = serde_json::from_str::<Vec<Value>>(&json) {
                    for item in list {
                        if let Some(p) = item.get("Path").and_then(|p| p.as_str()) {
                            if let Some(i) = probe(&Path::new(p).join(exe_name()), "PCL") {
                                found.push(i);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut seen = HashSet::new();
    found.retain(|j| seen.insert(j.path.to_ascii_lowercase()));
    found.sort_by(|a, b| b.major.cmp(&a.major));
    found
}

/// 选一个版本最高的 Java；处理器对版本要求不高，但新版 NeoForge 的工具要 21+
pub fn pick(list: &[JavaInfo], min_major: u32) -> Option<&JavaInfo> {
    list.iter().find(|j| j.major >= min_major).or_else(|| list.first().filter(|j| j.major == 0))
}

const RUNTIME_INDEX: &str =
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

fn platform_key() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("windows-x64")
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some("windows-arm64")
    } else if cfg!(all(windows, target_arch = "x86")) {
        Some("windows-x86")
    } else {
        None
    }
}

/// 下载 Mojang 官方 Java 运行时，返回 java 可执行文件路径。
/// component 是版本 JSON 里 javaVersion.component（jre-legacy = 8、java-runtime-gamma = 17、delta = 21……）；
/// 不指定就挑一个新的（装 Forge 跑处理器用）
pub async fn download_runtime(net: &Arc<Net>, data_dir: &Path, prog: &Arc<Progress>, component: Option<&str>) -> Result<PathBuf> {
    let platform = platform_key().ok_or_else(|| anyhow!("当前系统没有可下载的官方 Java 运行时"))?;
    let index: Value = net.get_json(RUNTIME_INDEX).await?;
    let plat = index.get(platform).ok_or_else(|| anyhow!("运行时索引里没有 {}", platform))?;
    let order: Vec<&str> = match component {
        Some(c) => vec![c],
        None => vec!["java-runtime-delta", "java-runtime-epsilon", "java-runtime-gamma"],
    };
    let (component, entry) = order
        .iter()
        .find_map(|c| plat.get(*c).and_then(|v| v.as_array()).and_then(|a| a.first()).map(|e| (*c, e)))
        .ok_or_else(|| anyhow!("Mojang 没有给这台电脑提供 {} 运行时", order.join(" / ")))?;
    let manifest_url = entry
        .get("manifest")
        .and_then(|m| m.get("url"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow!("运行时清单地址缺失"))?;
    let manifest: Value = net.get_json(manifest_url).await?;
    let root = data_dir.join("java").join(component);
    let mut jobs = vec![];
    for (rel, f) in manifest.get("files").and_then(|f| f.as_object()).into_iter().flatten() {
        let dest = crate::base::util::join_rel(&root, rel);
        match f.get("type").and_then(|t| t.as_str()) {
            Some("directory") => {
                std::fs::create_dir_all(&dest)?;
            }
            Some("file") => {
                if let Some(raw) = f.get("downloads").and_then(|d| d.get("raw")) {
                    jobs.push(FileJob {
                        url: raw.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
                        path: dest,
                        sha1: raw.get("sha1").and_then(|s| s.as_str()).map(String::from),
                        size: raw.get("size").and_then(|s| s.as_u64()),
                    });
                }
            }
            _ => {} // Windows 版运行时没有软链接
        }
    }
    crate::game::install::download_all(net, &jobs, prog).await?;
    // 写个 release 文件，下次检测能读到版本号
    let version = entry.get("version").and_then(|v| v.get("name")).and_then(|n| n.as_str()).unwrap_or("21");
    let _ = std::fs::write(root.join("release"), format!("JAVA_VERSION=\"{}\"\n", version));
    let java = root.join("bin").join(exe_name());
    java.is_file().then_some(java.clone()).with_context(|| format!("运行时下载完了但找不到 {}", java.display()))
}
