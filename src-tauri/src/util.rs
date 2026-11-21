//! 零散工具：Maven 坐标、规则判定、文件读写。

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::Value;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// 解析后的 Maven 坐标：`group:artifact:version[:classifier][@ext]`
#[derive(Debug, Clone)]
pub struct Coord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub ext: String,
}

impl Coord {
    pub fn parse(s: &str) -> Option<Coord> {
        let s = s.trim().trim_start_matches('[').trim_end_matches(']');
        let (body, ext) = match s.split_once('@') {
            Some((b, e)) => (b, e.to_string()),
            None => (s, "jar".to_string()),
        };
        let parts: Vec<&str> = body.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Coord {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).map(|c| c.to_string()),
            ext,
        })
    }

    /// 相对 libraries 目录的路径（用 `/` 分隔，可直接拼 URL）
    pub fn path(&self) -> String {
        let file = match &self.classifier {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, self.ext),
            None => format!("{}-{}.{}", self.artifact, self.version, self.ext),
        };
        format!("{}/{}/{}/{}", self.group.replace('.', "/"), self.artifact, self.version, file)
    }

    /// 去重用的键：同一个库不同版本视为同一项，natives 分类器区分开
    pub fn dedupe_key(&self) -> String {
        match &self.classifier {
            Some(c) => format!("{}:{}:{}", self.group, self.artifact, c),
            None => format!("{}:{}", self.group, self.artifact),
        }
    }
}

/// 目标平台。安卓启动器（ZL2 / FCL / Pojav）按 Linux 规则取库，LWJGL 由启动器自己替换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsTarget {
    Windows,
    Linux,
    Osx,
}

impl OsTarget {
    pub fn current() -> OsTarget {
        if cfg!(target_os = "windows") {
            OsTarget::Windows
        } else if cfg!(target_os = "macos") {
            OsTarget::Osx
        } else {
            OsTarget::Linux
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            OsTarget::Windows => "windows",
            OsTarget::Linux => "linux",
            OsTarget::Osx => "osx",
        }
    }
}

/// 版本 JSON 里的 `rules` 判定。带 features 的规则一律视为不满足（那是给启动参数用的）。
pub fn rules_allow(rules: Option<&Value>, os: OsTarget) -> bool {
    let Some(rules) = rules.and_then(|r| r.as_array()) else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let action_allow = rule.get("action").and_then(|a| a.as_str()) == Some("allow");
        if rule.get("features").is_some() {
            continue;
        }
        let matches = match rule.get("os") {
            None => true,
            Some(o) => {
                let name_ok = o.get("name").and_then(|n| n.as_str()).map_or(true, |n| n == os.name());
                // 只按 64 位 x86 取库：32 位和 ARM 的 natives 交给需要的启动器自己补
                let arch_ok = o
                    .get("arch")
                    .and_then(|a| a.as_str())
                    .map_or(true, |a| !matches!(a, "x86" | "arm64" | "aarch64" | "arm32"));
                name_ok && arch_ok
            }
        };
        if matches {
            allowed = action_allow;
        }
    }
    allowed
}

/// 版本名里不能出现的字符（Windows 文件名限制 + 启动器约定）
pub fn sanitize_version_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    cleaned.trim().trim_end_matches('.').to_string()
}

pub fn read_json(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).with_context(|| format!("读取 {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("解析 {}", path.display()))
}

pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text).with_context(|| format!("写入 {}", path.display()))
}

/// `a/b/c` 形式的相对路径拼到根目录下
pub fn join_rel(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for seg in rel.split('/').filter(|s| !s.is_empty()) {
        p.push(seg);
    }
    p
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}
