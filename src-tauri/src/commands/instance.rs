//! 实例页：读 Mod、存档、截图，启用 / 禁用 Mod，打开实例的文件夹
//!
//! 前端只传「哪个游戏目录（扫描时给的 id）的哪个版本」，路径由这里从扫描结果里查，
//! 不接受前端传来的任意路径。

use std::path::PathBuf;

use tauri::State;

use crate::base::platform;
use crate::instance::{self, InstanceFiles};
use crate::state::{err, AppState, CmdResult};

pub(crate) fn root_of(state: &AppState, dir: &str) -> CmdResult<PathBuf> {
    state
        .targets
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == dir && t.access == "direct")
        .map(|t| PathBuf::from(&t.path))
        .ok_or_else(|| "找不到这个游戏目录，请重新扫描".to_string())
}

#[tauri::command]
pub async fn instance_files(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<InstanceFiles> {
    let root = root_of(&state, &dir)?;
    tauri::async_runtime::spawn_blocking(move || instance::files(&root, &name)).await.map_err(|e| e.to_string())?.map_err(err)
}

#[tauri::command]
pub fn set_mod_enabled(state: State<'_, AppState>, dir: String, name: String, file: String, enabled: bool) -> CmdResult<String> {
    let root = root_of(&state, &dir)?;
    instance::set_mod_enabled(&root, &name, &file, enabled).map_err(err)
}

/// 打开实例的文件夹；`sub` 是 mods、saves、logs 这类子文件夹，空的就是游戏目录本身
#[tauri::command]
pub fn open_instance_dir(state: State<'_, AppState>, dir: String, name: String, sub: String) -> CmdResult<()> {
    let root = root_of(&state, &dir)?;
    instance::check_name(&name).map_err(err)?;
    let mut p = instance::game_dir(&root, &name).0;
    if !sub.is_empty() {
        // 可以是两层，比如 saves/新的世界；每一层都不能跑出实例目录
        for part in sub.split('/') {
            instance::check_name(part).map_err(err)?;
        }
        p = p.join(&sub);
        if !p.is_dir() {
            std::fs::create_dir_all(&p).map_err(|e| format!("建不了文件夹：{}", e))?;
        }
    }
    platform::open_path(&p.to_string_lossy()).map_err(err)
}

/// 打开原来的启动器（LeafStarter 自己能启动游戏之前，先用它玩）
#[tauri::command]
pub fn open_launcher(state: State<'_, AppState>, dir: String) -> CmdResult<()> {
    let exe = state
        .targets
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == dir)
        .and_then(|t| t.exe.clone())
        .ok_or_else(|| "没找到这个启动器的程序".to_string())?;
    platform::open_path(&exe).map_err(err)
}

#[tauri::command]
pub fn instance_config(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<crate::launch::Options> {
    let root = root_of(&state, &dir)?;
    instance::check_name(&name).map_err(err)?;
    Ok(instance::load_config(&root, &name))
}

#[tauri::command]
pub fn set_instance_config(state: State<'_, AppState>, dir: String, name: String, cfg: crate::launch::Options) -> CmdResult<()> {
    let root = root_of(&state, &dir)?;
    instance::save_config(&root, &name, &cfg).map_err(err)
}

#[tauri::command]
pub async fn rename_instance(state: State<'_, AppState>, dir: String, name: String, new_name: String) -> CmdResult<String> {
    let root = root_of(&state, &dir)?;
    tauri::async_runtime::spawn_blocking(move || instance::rename(&root, &name, &new_name)).await.map_err(|e| e.to_string())?.map_err(err)
}

#[tauri::command]
pub async fn copy_instance(state: State<'_, AppState>, dir: String, name: String, new_name: String) -> CmdResult<String> {
    let root = root_of(&state, &dir)?;
    tauri::async_runtime::spawn_blocking(move || instance::copy(&root, &name, &new_name)).await.map_err(|e| e.to_string())?.map_err(err)
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<()> {
    let root = root_of(&state, &dir)?;
    tauri::async_runtime::spawn_blocking(move || instance::delete(&root, &name)).await.map_err(|e| e.to_string())?.map_err(err)
}

/// 崩溃分析给出的一键修复
#[tauri::command]
pub async fn apply_fix(state: State<'_, AppState>, dir: String, name: String, fix: crate::launch::crash::Fix) -> CmdResult<String> {
    use crate::launch::crash::Fix;
    let root = root_of(&state, &dir)?;
    instance::check_name(&name).map_err(err)?;
    let mut cfg = instance::load_config(&root, &name);
    let msg = match fix {
        Fix::Memory { mb } => {
            cfg.memory_mb = Some(mb);
            format!("内存改成了 {} MB", mb)
        }
        Fix::JavaAuto => {
            cfg.java_path.clear();
            "改回了自动选 Java".to_string()
        }
        Fix::JavaMin { major } => {
            let list = crate::game::java::detect(&state.data_dir);
            let path = match list.iter().filter(|j| j.major >= major).min_by_key(|j| j.major) {
                Some(j) => j.path.clone(),
                None => {
                    let comp = match major {
                        0..=8 => "jre-legacy",
                        9..=16 => "java-runtime-alpha",
                        17..=20 => "java-runtime-gamma",
                        21..=24 => "java-runtime-delta",
                        _ => "java-runtime-epsilon",
                    };
                    let prog = std::sync::Arc::new(crate::base::net::Progress::default());
                    crate::game::java::download_runtime(&state.net, &state.data_dir, &prog, Some(comp)).await.map_err(err)?.to_string_lossy().into_owned()
                }
            };
            cfg.java_path = path;
            format!("这个实例改用 Java {}+", major)
        }
        Fix::DisableMods { files } => {
            for f in &files {
                instance::set_mod_enabled(&root, &name, f, false).map_err(err)?;
            }
            return Ok(format!("禁用了 {}", files.join("、")));
        }
    };
    instance::save_config(&root, &name, &cfg).map_err(err)?;
    Ok(msg)
}

/// 一个存档的信息：名字、种子、模式、版本、玩家在哪
#[tauri::command]
pub async fn world_info(state: State<'_, AppState>, dir: String, name: String, world: String) -> CmdResult<crate::game::world::WorldInfo> {
    let root = root_of(&state, &dir)?;
    instance::check_name(&name).map_err(err)?;
    instance::check_name(&world).map_err(err)?;
    let p = instance::game_dir(&root, &name).0.join("saves").join(&world);
    tauri::async_runtime::spawn_blocking(move || crate::game::world::info(&p)).await.map_err(|e| e.to_string())?.map_err(err)
}

/// 设置模板：把另一个实例的键位、视角、画面、语言和声音套到这个实例上
#[tauri::command]
pub fn apply_options(state: State<'_, AppState>, dir: String, name: String, from_dir: String, from_name: String, groups: Vec<String>) -> CmdResult<crate::game::options::Applied> {
    let (root, from_root) = (root_of(&state, &dir)?, root_of(&state, &from_dir)?);
    instance::check_name(&name).map_err(err)?;
    instance::check_name(&from_name).map_err(err)?;
    if groups.iter().any(|g| !crate::game::options::GROUPS.contains(&g.as_str())) {
        return Err("不认识的设置类别".into());
    }
    let mc = state.targets.lock().unwrap().iter().find(|t| t.id == dir).and_then(|t| t.versions.iter().find(|v| v.name == name)).and_then(|v| v.mc.clone());
    let src = instance::game_dir(&from_root, &from_name).0.join("options.txt");
    let dst = instance::game_dir(&root, &name).0.join("options.txt");
    if src == dst {
        return Err("这两个实例用的是同一份设置（都没开版本隔离），不用套".into());
    }
    crate::game::options::apply(&src, &dst, mc.as_deref(), &groups).map_err(err)
}

/// 实例里每个 Mod 的元数据（id、名字、版本、作者、前置、链接），和缺了哪些必需的前置
#[tauri::command]
pub async fn instance_mods(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<serde_json::Value> {
    let root = root_of(&state, &dir)?;
    instance::check_name(&name).map_err(err)?;
    let mods_dir = instance::game_dir(&root, &name).0.join("mods");
    let loader = loader_of(&state, &dir, &name);
    tauri::async_runtime::spawn_blocking(move || {
        let mods = crate::game::modmeta::read_dir(&mods_dir, &loader);
        let missing = crate::game::modmeta::missing_deps(&mods, &loader);
        serde_json::json!({ "mods": mods, "missing": missing })
    })
    .await
    .map_err(|e| e.to_string())
}

/// 扫描时认出的加载器（Forge / NeoForge / Fabric / Quilt，原版是空的）
pub(crate) fn loader_of(state: &AppState, dir: &str, name: &str) -> String {
    state.targets.lock().unwrap().iter().find(|t| t.id == dir).and_then(|t| t.versions.iter().find(|v| v.name == name)).map(|v| v.loader.clone()).unwrap_or_default()
}
