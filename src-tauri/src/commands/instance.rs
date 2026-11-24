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
        instance::check_name(&sub).map_err(err)?;
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
