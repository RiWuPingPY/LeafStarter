//! 实例页的 Mod 组：列出、新建、切换、改名、删除。只给版本隔离的实例用（见 game/modgroups.rs）。

use std::path::PathBuf;

use tauri::State;

use super::instance::root_of;
use crate::game::modgroups;
use crate::instance;
use crate::state::{err, AppState, CmdResult};

/// 实例的游戏目录；不隔离的实例不能分组
fn gd_of(state: &AppState, dir: &str, name: &str) -> CmdResult<PathBuf> {
    let root = root_of(state, dir)?;
    instance::check_name(name).map_err(err)?;
    let (gd, isolated) = instance::game_dir(&root, name);
    if !isolated {
        return Err("这个实例没开版本隔离，和别的实例共用 mods 文件夹，不能分组".into());
    }
    Ok(gd)
}

/// 游戏开着或者正在二分排查 Mod 时不能动 mods 文件夹
pub(crate) fn ensure_idle(state: &AppState, dir: &str, name: &str) -> CmdResult<()> {
    let key = format!("{dir}/{name}");
    if state.runs.lock().unwrap().values().any(|r| r.key == key) {
        return Err("游戏还开着，关掉再切 Mod 组".into());
    }
    if state.bisects.lock().unwrap().contains_key(&key) {
        return Err("正在排查 Mod，排查完再切".into());
    }
    Ok(())
}

#[tauri::command]
pub fn mod_groups(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<serde_json::Value> {
    let root = root_of(&state, &dir)?;
    instance::check_name(&name).map_err(err)?;
    let (gd, isolated) = instance::game_dir(&root, &name);
    let mut v = modgroups::view(&gd);
    v["isolated"] = isolated.into();
    Ok(v)
}

#[tauri::command]
pub async fn mod_group_create(state: State<'_, AppState>, dir: String, name: String, group: String, copy: bool) -> CmdResult<String> {
    let gd = gd_of(&state, &dir, &name)?;
    tauri::async_runtime::spawn_blocking(move || modgroups::create(&gd, &group, copy, None)).await.map_err(|e| e.to_string())?.map_err(err)
}

#[tauri::command]
pub async fn mod_group_switch(state: State<'_, AppState>, dir: String, name: String, id: String) -> CmdResult<()> {
    let gd = gd_of(&state, &dir, &name)?;
    ensure_idle(&state, &dir, &name)?;
    tauri::async_runtime::spawn_blocking(move || modgroups::switch(&gd, &id)).await.map_err(|e| e.to_string())?.map_err(err)
}

#[tauri::command]
pub fn mod_group_rename(state: State<'_, AppState>, dir: String, name: String, id: String, group: String) -> CmdResult<()> {
    let gd = gd_of(&state, &dir, &name)?;
    modgroups::rename(&gd, &id, &group).map_err(err)
}

#[tauri::command]
pub async fn mod_group_delete(state: State<'_, AppState>, dir: String, name: String, id: String) -> CmdResult<()> {
    let gd = gd_of(&state, &dir, &name)?;
    tauri::async_runtime::spawn_blocking(move || modgroups::delete(&gd, &id)).await.map_err(|e| e.to_string())?.map_err(err)
}
