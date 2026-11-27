//! Mod 组：一个实例里存几套 Mod，一键切换。
//! 联机时朋友的游戏版本一样、只是 Mod 不一样，就在这个实例里新建一组朋友的 Mod，不用为此重下整个游戏。
//!
//! 存法（只给版本隔离的实例用：不隔离的实例和别的实例共用一个 mods 文件夹，切了会连累别人）：
//! - 正在用的那组就是实例的 `mods/`；
//! - 其他组放在 `versions/<实例>/LeafStarter/modgroups/<组 id>/`；
//! - 组的名字、哪组在用，记在 `versions/<实例>/LeafStarter/modgroups.json`。
//! 切换就是两次改名（同一个盘里，瞬间完成）。复制一组时用硬链接，不占双份空间（硬链接不了就复制）。
//! config 文件夹几组共用：没装的 Mod 留下的配置不碍事。

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub created: u64,
    /// 从哪来的：比如「朋友的邀请码」
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Groups {
    pub active: String,
    pub groups: Vec<Group>,
}

impl Default for Groups {
    fn default() -> Self {
        Groups { active: "default".into(), groups: vec![Group { id: "default".into(), name: "默认".into(), created: 0, source: None }] }
    }
}

fn meta_path(gd: &Path) -> PathBuf {
    gd.join("LeafStarter").join("modgroups.json")
}

fn store(gd: &Path, id: &str) -> PathBuf {
    gd.join("LeafStarter").join("modgroups").join(id)
}

pub fn load(gd: &Path) -> Groups {
    std::fs::read(meta_path(gd)).ok().and_then(|b| serde_json::from_slice::<Groups>(&b).ok()).filter(|g| g.groups.iter().any(|x| x.id == g.active)).unwrap_or_default()
}

fn save(gd: &Path, g: &Groups) -> Result<()> {
    crate::base::util::write_json(&meta_path(gd), &serde_json::to_value(g)?)
}

/// 这组的 Mod 现在放在哪
pub fn dir_of(gd: &Path, g: &Groups, id: &str) -> PathBuf {
    if g.active == id {
        gd.join("mods")
    } else {
        store(gd, id)
    }
}

fn is_mod(p: &Path) -> bool {
    let n = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    p.is_file() && (n.ends_with(".jar") || n.ends_with(".jar.disabled") || n.ends_with(".zip") || n.ends_with(".zip.disabled"))
}

/// 文件夹里的 Mod 文件名（含 .disabled）
pub fn files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| is_mod(&e.path())).map(|e| e.file_name().to_string_lossy().into_owned()).collect()
}

/// 给界面看：每组叫什么、几个 Mod、是不是正在用
pub fn view(gd: &Path) -> Value {
    let g = load(gd);
    let list: Vec<Value> = g
        .groups
        .iter()
        .map(|x| {
            let dir = dir_of(gd, &g, &x.id);
            let all = files(&dir);
            let on = all.iter().filter(|f| !f.ends_with(".disabled")).count();
            json!({ "id": x.id, "name": x.name, "active": x.id == g.active, "count": all.len(), "on": on, "source": x.source })
        })
        .collect();
    json!({ "active": g.active, "groups": list })
}

fn new_id() -> String {
    format!("g{:x}", crate::base::util::now_ms())
}

fn check_name(g: &Groups, name: &str, except: Option<&str>) -> Result<String> {
    let name: String = name.trim().chars().take(30).collect();
    if name.is_empty() {
        bail!("起个名字");
    }
    if g.groups.iter().any(|x| x.name == name && Some(x.id.as_str()) != except) {
        bail!("已经有叫「{name}」的组了");
    }
    Ok(name)
}

/// 把一个文件放进另一个文件夹：同一个盘先试硬链接（不占空间），不行就复制
pub fn link_or_copy(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        return Ok(());
    }
    if std::fs::hard_link(from, to).is_err() {
        std::fs::copy(from, to).with_context(|| format!("复制 {} 失败", from.display()))?;
    }
    Ok(())
}

/// 不重名的组名：重了就在后面加 2、3……
pub fn unique_name(gd: &Path, base: &str) -> String {
    let g = load(gd);
    let base: String = base.trim().chars().take(26).collect();
    (1..100).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") }).find(|n| !g.groups.iter().any(|x| &x.name == n)).unwrap_or(base)
}

/// 没建成的组直接删掉（文件是刚下的一半，不进回收站）
pub fn discard(gd: &Path, id: &str) {
    let mut g = load(gd);
    if g.active == id {
        return;
    }
    let _ = std::fs::remove_dir_all(store(gd, id));
    g.groups.retain(|x| x.id != id);
    let _ = save(gd, &g);
}

/// 新建一组：空的，或者把现在用的这组复制一份。返回新组的 id
pub fn create(gd: &Path, name: &str, copy_current: bool, source: Option<String>) -> Result<String> {
    let mut g = load(gd);
    let name = check_name(&g, name, None)?;
    let id = new_id();
    let dir = store(gd, &id);
    std::fs::create_dir_all(&dir)?;
    if copy_current {
        let cur = gd.join("mods");
        for f in files(&cur) {
            link_or_copy(&cur.join(&f), &dir.join(&f))?;
        }
    }
    g.groups.push(Group { id: id.clone(), name, created: crate::base::util::now_ms(), source });
    save(gd, &g)?;
    Ok(id)
}

/// 切到另一组：现在的 mods 收进它自己的格子，目标组搬到 mods
pub fn switch(gd: &Path, id: &str) -> Result<()> {
    let mut g = load(gd);
    if g.active == id {
        return Ok(());
    }
    if !g.groups.iter().any(|x| x.id == id) {
        bail!("没有这个 Mod 组");
    }
    let mods = gd.join("mods");
    let park = store(gd, &g.active);
    let target = store(gd, id);
    std::fs::create_dir_all(park.parent().unwrap())?;
    if park.exists() {
        // 上次切换没收拾干净（不该发生）：空的就删掉，不空就别冒险
        if std::fs::read_dir(&park).map(|mut d| d.next().is_none()).unwrap_or(false) {
            std::fs::remove_dir(&park)?;
        } else {
            bail!("{} 已经有东西了，先手动看一下", park.display());
        }
    }
    if mods.exists() {
        std::fs::rename(&mods, &park).context("mods 文件夹正被占用（游戏还开着？），关掉再切")?;
    } else {
        std::fs::create_dir_all(&park)?;
    }
    if target.exists() {
        if let Err(e) = std::fs::rename(&target, &mods) {
            // 退回原样
            let _ = std::fs::rename(&park, &mods);
            return Err(e).context("切换失败，已经恢复原样");
        }
    } else {
        std::fs::create_dir_all(&mods)?;
    }
    g.active = id.to_string();
    save(gd, &g)
}

pub fn rename(gd: &Path, id: &str, name: &str) -> Result<()> {
    let mut g = load(gd);
    let name = check_name(&g, name, Some(id))?;
    let x = g.groups.iter_mut().find(|x| x.id == id).context("没有这个 Mod 组")?;
    x.name = name;
    save(gd, &g)
}

/// 删掉一组（不能删正在用的）：文件进回收站
pub fn delete(gd: &Path, id: &str) -> Result<()> {
    let mut g = load(gd);
    if g.active == id {
        bail!("正在用的组不能删，先切到别的组");
    }
    let dir = store(gd, id);
    if dir.exists() {
        if files(&dir).is_empty() && std::fs::read_dir(&dir).map(|mut d| d.next().is_none()).unwrap_or(false) {
            std::fs::remove_dir(&dir)?;
        } else {
            // 放不进回收站就报错，不悄悄永久删除
            crate::instance::to_recycle_bin(&dir)?;
        }
    }
    g.groups.retain(|x| x.id != id);
    save(gd, &g)
}

/// 实例里所有组的所有 Mod 文件：SHA-1 → 路径（建新组时，已经有的文件直接链过去，不用再下）
pub fn all_files_by_sha1(gd: &Path) -> std::collections::HashMap<String, PathBuf> {
    let g = load(gd);
    let mut out = std::collections::HashMap::new();
    for x in &g.groups {
        let dir = dir_of(gd, &g, &x.id);
        for f in files(&dir) {
            let p = dir.join(&f);
            if let Ok(h) = crate::base::net::sha1_file(&p) {
                out.entry(h).or_insert(p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_switch_delete() {
        let gd = std::env::temp_dir().join(format!("ls-mg-{}", crate::base::util::now_ms()));
        std::fs::create_dir_all(gd.join("mods")).unwrap();
        std::fs::write(gd.join("mods").join("a.jar"), b"a").unwrap();
        std::fs::write(gd.join("mods").join("b.jar.disabled"), b"b").unwrap();
        // 复制一组：两个文件都在
        let copy = create(&gd, "朋友的", true, None).unwrap();
        assert_eq!(files(&store(&gd, &copy)).len(), 2);
        // 空的一组
        let empty = create(&gd, "空的", false, None).unwrap();
        assert!(create(&gd, "空的", false, None).is_err());
        // 切到空的：mods 变空，原来的收进 default
        switch(&gd, &empty).unwrap();
        assert!(files(&gd.join("mods")).is_empty());
        assert_eq!(files(&store(&gd, "default")).len(), 2);
        assert_eq!(load(&gd).active, empty);
        // 正在用的不能删；切回默认后能删
        assert!(delete(&gd, &empty).is_err());
        switch(&gd, "default").unwrap();
        assert_eq!(files(&gd.join("mods")).len(), 2);
        delete(&gd, &empty).unwrap();
        rename(&gd, &copy, "朋友的 Mod").unwrap();
        let v = view(&gd);
        assert_eq!(v["groups"].as_array().unwrap().len(), 2);
        assert_eq!(v["groups"][0]["on"], 1);
        let _ = std::fs::remove_dir_all(&gd);
    }
}
