//! 实例里的文件：Mod、存档、截图，以及启用 / 禁用 Mod。
//!
//! 实例 = 游戏目录（`.minecraft`）里的一个版本。开了版本隔离（PCL、HMCL 的常见做法）时，
//! Mod 和存档放在 `versions/<名字>/` 下面，否则放在游戏目录根上。
//! 启用 / 禁用 Mod 和 PCL、HMCL 一样：禁用就是在文件名后面加 `.disabled`。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

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

// ---------- 实例设置：跟着版本文件夹走（versions/<名字>/LeafStarter/instance.json） ----------

fn config_path(root: &Path, name: &str) -> PathBuf {
    root.join("versions").join(name).join("LeafStarter").join("instance.json")
}

pub fn load_config(root: &Path, name: &str) -> crate::launch::Options {
    crate::base::util::read_json(&config_path(root, name)).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

pub fn save_config(root: &Path, name: &str, cfg: &crate::launch::Options) -> Result<()> {
    check_name(name)?;
    let p = config_path(root, name);
    std::fs::create_dir_all(p.parent().unwrap())?;
    crate::base::util::write_json(&p, &serde_json::to_value(cfg)?)
}

// ---------- 改名、复制、删除 ----------

/// 新版本名：去掉文件名里不能有的字符；不能和已有的重名
pub fn new_name_ok(root: &Path, name: &str) -> Result<String> {
    let n = crate::base::util::sanitize_version_name(name);
    if n.is_empty() {
        bail!("名字不能是空的");
    }
    check_name(&n)?;
    if root.join("versions").join(&n).exists() {
        bail!("这个游戏目录里已经有叫「{}」的版本了", n);
    }
    Ok(n)
}

/// 把版本文件夹里的 <旧名>.json / <旧名>.jar / natives 改成新名字，JSON 里的 id 也改掉
fn rename_inside(vdir: &Path, old: &str, new: &str) -> Result<()> {
    let json_old = vdir.join(format!("{}.json", old));
    if json_old.is_file() {
        let mut v: Value = crate::base::util::read_json(&json_old)?;
        if let Some(o) = v.as_object_mut() {
            o.insert("id".into(), Value::String(new.to_string()));
            // 原来靠自己文件夹里的 jar 的，改名后 jar 跟着改名；靠 jar 字段指向别的版本的不用动
        }
        crate::base::util::write_json(&vdir.join(format!("{}.json", new)), &v)?;
        std::fs::remove_file(&json_old)?;
    }
    let jar_old = vdir.join(format!("{}.jar", old));
    if jar_old.is_file() {
        std::fs::rename(&jar_old, vdir.join(format!("{}.jar", new)))?;
    }
    let _ = std::fs::remove_dir_all(vdir.join(format!("{}-natives", old)));
    Ok(())
}

/// 靠这个版本启动的别的版本（JSON 里 inheritsFrom 或 jar 指向它，比如 Fabric 实例靠原版）：(版本名, JSON 路径)
fn dependents(root: &Path, name: &str) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    for e in std::fs::read_dir(root.join("versions")).into_iter().flatten().flatten() {
        let other = e.file_name().to_string_lossy().into_owned();
        if other == name {
            continue;
        }
        let p = e.path().join(format!("{other}.json"));
        let Ok(v) = crate::base::util::read_json(&p) else { continue };
        if v["inheritsFrom"].as_str() == Some(name) || v["jar"].as_str() == Some(name) {
            out.push((other, p));
        }
    }
    out
}

pub fn rename(root: &Path, name: &str, new_name: &str) -> Result<String> {
    check_name(name)?;
    let new = new_name_ok(root, new_name)?;
    let from = root.join("versions").join(name);
    let to = root.join("versions").join(&new);
    let deps = dependents(root, name);
    std::fs::rename(&from, &to).with_context(|| format!("改不了名（游戏开着的话先关掉）：{}", name))?;
    rename_inside(&to, name, &new)?;
    // 靠它启动的版本跟着改指向，不然改完名它们就起不来了
    for (_, p) in deps {
        if let Ok(mut v) = crate::base::util::read_json(&p) {
            for k in ["inheritsFrom", "jar"] {
                if v[k].as_str() == Some(name) {
                    v[k] = Value::String(new.clone());
                }
            }
            crate::base::util::write_json(&p, &v)?;
        }
    }
    Ok(new)
}

pub fn copy(root: &Path, name: &str, new_name: &str) -> Result<String> {
    check_name(name)?;
    let new = new_name_ok(root, new_name)?;
    let from = root.join("versions").join(name);
    let to = root.join("versions").join(&new);
    copy_dir(&from, &to, &format!("{}-natives", name))?;
    rename_inside(&to, name, &new)?;
    Ok(new)
}

fn copy_dir(from: &Path, to: &Path, skip: &str) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let name = e.file_name();
        if name.to_string_lossy() == skip {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst, "")?;
        } else {
            std::fs::copy(&src, &dst).with_context(|| format!("复制失败：{}", src.display()))?;
        }
    }
    Ok(())
}

/// 删除版本文件夹：放进回收站，后悔了还能找回来
pub fn delete(root: &Path, name: &str) -> Result<()> {
    check_name(name)?;
    let dir = root.join("versions").join(name);
    if !dir.is_dir() {
        bail!("找不到这个版本的文件夹");
    }
    let deps = dependents(root, name);
    if !deps.is_empty() {
        let names: Vec<String> = deps.into_iter().map(|d| format!("「{}」", d.0)).collect();
        bail!("{} 要靠「{name}」才能启动，删了它们就起不来了。先删它们，或者留着这个", names.join("、"));
    }
    to_recycle_bin(&dir)
}

#[cfg(windows)]
pub fn to_recycle_bin(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    struct ShFileOp {
        hwnd: *mut u8,
        func: u32,
        from: *const u16,
        to: *const u16,
        flags: u16,
        aborted: i32,
        mappings: *mut u8,
        title: *const u16,
    }
    #[link(name = "shell32")]
    extern "system" {
        fn SHFileOperationW(op: *mut ShFileOp) -> i32;
    }
    const FO_DELETE: u32 = 3;
    // 放回收站、不弹确认、不显示进度、出错不弹窗；但这个盘没有回收站、要永久删除时一定先问
    const FLAGS: u16 = 0x0040 | 0x0010 | 0x0004 | 0x0400 | 0x4000;
    // 这个接口只认反斜杠的完整路径；手动加的目录可能是 D:/a/b 这种写法
    let path = PathBuf::from(path.to_string_lossy().replace('/', "\\"));
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    from.extend([0, 0]);
    let mut op = ShFileOp { hwnd: std::ptr::null_mut(), func: FO_DELETE, from: from.as_ptr(), to: std::ptr::null(), flags: FLAGS, aborted: 0, mappings: std::ptr::null_mut(), title: std::ptr::null() };
    // SAFETY: from 是以两个 0 结尾的宽字符串，在调用期间有效
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 || op.aborted != 0 {
        bail!("放不进回收站（错误 {}）。游戏开着的话先关掉", r);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn to_recycle_bin(path: &Path) -> Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod manage_tests {
    use super::*;

    #[test]
    fn rename_copy_and_config() {
        let root = std::env::temp_dir().join(format!("leafstarter-test-manage-{}", std::process::id()));
        let v = root.join("versions").join("老名字");
        std::fs::create_dir_all(v.join("mods")).unwrap();
        std::fs::write(v.join("老名字.json"), r#"{"id":"老名字","mainClass":"x"}"#).unwrap();
        std::fs::write(v.join("老名字.jar"), b"jar").unwrap();
        std::fs::write(v.join("mods").join("a.jar"), b"m").unwrap();

        let cfg = crate::launch::Options { memory_mb: Some(4096), jvm_args: "-XX:+UseZGC".into(), ..Default::default() };
        save_config(&root, "老名字", &cfg).unwrap();
        assert_eq!(load_config(&root, "老名字").memory_mb, Some(4096));

        assert_eq!(copy(&root, "老名字", "副本").unwrap(), "副本");
        let c = root.join("versions").join("副本");
        assert!(c.join("副本.json").is_file() && c.join("副本.jar").is_file() && c.join("mods").join("a.jar").is_file());
        assert!(std::fs::read_to_string(c.join("副本.json")).unwrap().contains("\"副本\""));
        assert_eq!(load_config(&root, "副本").jvm_args, "-XX:+UseZGC");

        assert!(rename(&root, "老名字", "副本").is_err());
        // 靠它启动的版本跟着改指向；有人靠它时不让删
        let f = root.join("versions").join("模组版");
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("模组版.json"), r#"{"id":"模组版","inheritsFrom":"老名字"}"#).unwrap();
        assert!(delete(&root, "老名字").is_err());
        assert_eq!(rename(&root, "老名字", "新名字?").unwrap(), "新名字_");
        assert!(root.join("versions").join("新名字_").join("新名字_.json").is_file());
        assert!(!root.join("versions").join("老名字").exists());
        assert!(std::fs::read_to_string(f.join("模组版.json")).unwrap().contains("新名字_"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
