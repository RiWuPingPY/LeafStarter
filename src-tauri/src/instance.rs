//! 实例里的文件：Mod、存档、截图，以及启用 / 禁用 Mod。
//!
//! 实例 = 游戏目录（`.minecraft`）里的一个版本。开了版本隔离（PCL、HMCL 的常见做法）时，
//! Mod 和存档放在 `versions/<名字>/` 下面，否则放在游戏目录根上。
//! 启用 / 禁用 Mod 和 PCL、HMCL 一样：禁用就是在文件名后面加 `.disabled`。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::game::mojang::ISOLATION_DIRS;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceFiles {
    /// 游戏实际运行的目录
    pub game_dir: String,
    pub isolated: bool,
    pub mods: Vec<ModFile>,
    pub worlds: Vec<WorldDir>,
    pub screenshots: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file: String,
    pub enabled: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldDir {
    /// 存档文件夹名（游戏里显示的名字在 level.dat 里，后面读 NBT 时再换）
    pub name: String,
    /// 最后一次保存的时间（毫秒）
    pub modified: u64,
}

/// 版本名只能是一层文件夹名，防止 `..` 之类跑出游戏目录
pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', ':']) {
        bail!("版本名不对：{}", name);
    }
    Ok(())
}

/// 这个版本实际的游戏目录，和有没有开版本隔离
pub fn game_dir(root: &Path, name: &str) -> (PathBuf, bool) {
    let vdir = root.join("versions").join(name);
    if ISOLATION_DIRS.iter().any(|d| vdir.join(d).is_dir()) {
        (vdir, true)
    } else {
        (root.to_path_buf(), false)
    }
}

fn mtime_ms(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn is_mod_file(f: &str) -> bool {
    let base = f.strip_suffix(".disabled").unwrap_or(f).to_ascii_lowercase();
    base.ends_with(".jar") || base.ends_with(".zip")
}

pub fn files(root: &Path, name: &str) -> Result<InstanceFiles> {
    check_name(name)?;
    let (dir, isolated) = game_dir(root, name);

    let mut mods: Vec<ModFile> = std::fs::read_dir(dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().into_owned();
            is_mod_file(&file).then(|| ModFile {
                enabled: !file.ends_with(".disabled"),
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                file,
            })
        })
        .collect();
    mods.sort_by_key(|m| m.file.to_lowercase());

    let mut worlds: Vec<WorldDir> = std::fs::read_dir(dir.join("saves"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("level.dat").is_file())
        .map(|e| WorldDir { name: e.file_name().to_string_lossy().into_owned(), modified: mtime_ms(&e.path().join("level.dat")) })
        .collect();
    worlds.sort_by(|a, b| b.modified.cmp(&a.modified));

    let screenshots = std::fs::read_dir(dir.join("screenshots"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(".png"))
        .count();

    Ok(InstanceFiles { game_dir: dir.to_string_lossy().into_owned(), isolated, mods, worlds, screenshots })
}

/// 启用或禁用一个 Mod，返回改名后的文件名
pub fn set_mod_enabled(root: &Path, name: &str, file: &str, enabled: bool) -> Result<String> {
    check_name(name)?;
    check_name(file)?;
    if !is_mod_file(file) {
        bail!("不是 Mod 文件：{}", file);
    }
    let mods = game_dir(root, name).0.join("mods");
    let from = mods.join(file);
    if !from.is_file() {
        bail!("找不到这个 Mod 文件，可能已经被别的程序改过了：{}", file);
    }
    let to_name = match (enabled, file.strip_suffix(".disabled")) {
        (true, Some(base)) => base.to_string(),
        (false, None) => format!("{}.disabled", file),
        _ => return Ok(file.to_string()), // 已经是想要的状态
    };
    let to = mods.join(&to_name);
    if to.exists() {
        bail!("「{}」已经存在，先处理一下重名的文件", to_name);
    }
    std::fs::rename(&from, &to).with_context(|| format!("改不了文件名：{}（游戏开着的话先关掉）", file))?;
    Ok(to_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("leafstarter-test-inst-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn reads_isolated_instance_and_toggles_mods() {
        let root = scratch();
        let v = root.join("versions").join("生存");
        std::fs::create_dir_all(v.join("mods")).unwrap();
        std::fs::create_dir_all(v.join("saves").join("主世界")).unwrap();
        std::fs::write(v.join("saves").join("主世界").join("level.dat"), b"x").unwrap();
        std::fs::create_dir_all(v.join("saves").join("不是存档")).unwrap();
        std::fs::write(v.join("mods").join("sodium.jar"), b"x").unwrap();
        std::fs::write(v.join("mods").join("jade.jar.disabled"), b"x").unwrap();
        std::fs::write(v.join("mods").join("readme.txt"), b"x").unwrap();

        let f = files(&root, "生存").unwrap();
        assert!(f.isolated);
        assert_eq!(f.mods.iter().map(|m| (m.file.as_str(), m.enabled)).collect::<Vec<_>>(), [("jade.jar.disabled", false), ("sodium.jar", true)]);
        assert_eq!(f.worlds.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["主世界"]);

        assert_eq!(set_mod_enabled(&root, "生存", "sodium.jar", false).unwrap(), "sodium.jar.disabled");
        assert_eq!(set_mod_enabled(&root, "生存", "jade.jar.disabled", true).unwrap(), "jade.jar");
        assert!(v.join("mods").join("sodium.jar.disabled").is_file() && v.join("mods").join("jade.jar").is_file());

        // 不许跑出游戏目录
        assert!(files(&root, "..").is_err());
        assert!(set_mod_enabled(&root, "生存", "../x.jar", true).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn not_isolated_uses_root() {
        let root = scratch();
        std::fs::create_dir_all(root.join("versions").join("1.20.1")).unwrap();
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::write(root.join("mods").join("a.jar"), b"x").unwrap();
        let f = files(&root, "1.20.1").unwrap();
        assert!(!f.isolated);
        assert_eq!(f.mods.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
