//! 世界工具（M13，ROADMAP 5.16）：地图、查找已经生成的建筑、不进游戏改世界设置

use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::State;

use crate::game::region;
use crate::state::{err, AppState, CmdResult};

fn world_dir(st: &AppState, dir: &str, name: &str, world: &str) -> CmdResult<(PathBuf, PathBuf)> {
    crate::instance::check_name(name).map_err(err)?;
    crate::instance::check_name(world).map_err(err)?;
    let root = crate::commands::instance::root_of(st, dir)?;
    let gd = crate::instance::game_dir(&root, name).0;
    let wd = gd.join("saves").join(world);
    if !wd.join("level.dat").is_file() {
        return Err("找不到这个世界".into());
    }
    Ok((gd, wd))
}

fn dim_ok(dim: &str) -> CmdResult<()> {
    if ["overworld", "nether", "end"].contains(&dim) {
        Ok(())
    } else {
        Err("不认识的维度".into())
    }
}

/// 有哪些 region、出生点和玩家在哪（地图的标记）
#[tauri::command]
pub async fn world_map_regions(state: State<'_, AppState>, dir: String, name: String, world: String, dim: String) -> CmdResult<Value> {
    dim_ok(&dim)?;
    let (_, wd) = world_dir(&state, &dir, &name, &world)?;
    tauri::async_runtime::spawn_blocking(move || {
        let regions = region::list(&wd, &dim);
        let info = crate::game::world::info(&wd).ok();
        let pdim = match dim.as_str() {
            "nether" => "the_nether",
            "end" => "the_end",
            _ => "overworld",
        };
        let player = info.as_ref().and_then(|i| i.player.as_ref()).filter(|p| p.dimension == pdim).map(|p| [p.pos[0], p.pos[2]]);
        let spawn = if dim == "overworld" { info.as_ref().and_then(|i| i.spawn).map(|s| [s[0], s[2]]) } else { None };
        json!({ "regions": regions, "player": player, "spawn": spawn })
    })
    .await
    .map_err(|e| e.to_string())
}

/// 一块 512×512 的地图（PNG 的 base64）。按 region 文件的修改时间缓存在数据目录里
#[tauri::command]
pub async fn world_map_tile(state: State<'_, AppState>, dir: String, name: String, world: String, dim: String, x: i32, z: i32) -> CmdResult<String> {
    dim_ok(&dim)?;
    let (gd, wd) = world_dir(&state, &dir, &name, &world)?;
    let key = crate::commands::sync::world_key(&gd, &world);
    let cache = state.data_dir.join("mapcache").join(key.replace(['/', '\\', ':'], "_")).join(&dim);
    tauri::async_runtime::spawn_blocking(move || {
        let src = region::region_dir(&wd, &dim).join(format!("r.{x}.{z}.mca"));
        let mt = std::fs::metadata(&src).and_then(|m| m.modified()).map_err(|_| "没有这块地图".to_string())?;
        let stamp = mt.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let cp = cache.join(format!("r.{x}.{z}.{stamp}.png"));
        let png = match std::fs::read(&cp) {
            Ok(b) => b,
            Err(_) => {
                let rgba = region::render(&src, &dim).map_err(err)?;
                let b = region::png(&rgba, 512, 512).map_err(err)?;
                let _ = std::fs::create_dir_all(&cache);
                // 同一块的旧缓存删掉
                for e in std::fs::read_dir(&cache).into_iter().flatten().flatten() {
                    if e.file_name().to_string_lossy().starts_with(&format!("r.{x}.{z}.")) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
                let _ = std::fs::write(&cp, &b);
                b
            }
        };
        Ok(crate::base::util::base64(&png))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 已经生成的区块里记着的建筑，按离出生点（或玩家）的远近排
#[tauri::command]
pub async fn world_structures(state: State<'_, AppState>, dir: String, name: String, world: String, dim: String) -> CmdResult<Value> {
    dim_ok(&dim)?;
    let (_, wd) = world_dir(&state, &dir, &name, &world)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut list = region::structures(&wd, &dim).map_err(err)?;
        let info = crate::game::world::info(&wd).ok();
        let origin = info.as_ref().and_then(|i| i.spawn).map(|s| (s[0] as f64, s[2] as f64)).unwrap_or((0.0, 0.0));
        let dist = |s: &region::Structure| ((s.x as f64 - origin.0).powi(2) + (s.z as f64 - origin.1).powi(2)).sqrt();
        list.sort_by(|a, b| dist(a).total_cmp(&dist(b)));
        Ok(json!(list.iter().map(|s| json!({ "id": s.id, "x": s.x, "z": s.z, "dist": dist(s).round() })).collect::<Vec<_>>()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 游戏规则（只有老写法的存档能读到）
#[tauri::command]
pub fn world_rules(state: State<'_, AppState>, dir: String, name: String, world: String) -> CmdResult<Value> {
    let (_, wd) = world_dir(&state, &dir, &name, &world)?;
    Ok(json!(crate::game::world::rules(&wd).into_iter().collect::<std::collections::BTreeMap<_, _>>()))
}

/// 改世界设置：游戏开着这个实例时不让改；改之前存一份快照（能在「历史」里退回）
#[tauri::command]
pub async fn world_edit(state: State<'_, AppState>, dir: String, name: String, world: String, patch: Value) -> CmdResult<Value> {
    let key_inst = format!("{dir}/{name}");
    if state.runs.lock().unwrap().values().any(|r| r.key == key_inst) {
        return Err("游戏还开着，先关掉再改".into());
    }
    let (gd, wd) = world_dir(&state, &dir, &name, &world)?;
    let key = crate::commands::sync::world_key(&gd, &world);
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        store.snapshot(&key, &wd, "这台电脑", "manual", 5).map_err(err)?;
        let changed = crate::game::world::edit(&wd, &patch).map_err(err)?;
        Ok(json!({ "changed": changed }))
    })
    .await
    .map_err(|e| e.to_string())?
}
