//! 存档历史：列出、现在存一份、退回到某一份。游戏退出后自动给这次动过的世界存一份。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tauri::State;

use crate::commands::instance::root_of;
use crate::state::{err, AppState, CmdResult};

/// 每个世界在电脑上留几份
pub const KEEP: usize = 5;

/// 历史按「哪个游戏目录的哪个世界」分开存
pub fn world_key(game_dir: &Path, world: &str) -> String {
    let h: String = Sha1::digest(game_dir.to_string_lossy().to_lowercase().as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("{h}-{world}")
}

fn world_dir(state: &AppState, dir: &str, name: &str, world: &str) -> CmdResult<(PathBuf, String)> {
    let root = root_of(state, dir)?;
    crate::instance::check_name(name).map_err(err)?;
    crate::instance::check_name(world).map_err(err)?;
    let gd = crate::instance::game_dir(&root, name).0;
    Ok((gd.join("saves").join(world), world_key(&gd, world)))
}

fn view(s: &crate::sync::store::Snapshot) -> Value {
    json!({ "id": s.id, "at": s.at, "device": s.device, "reason": s.reason, "size": s.size(), "files": s.files.len() })
}

#[tauri::command]
pub async fn world_history(state: State<'_, AppState>, dir: String, name: String, world: String) -> CmdResult<Vec<Value>> {
    let (_, key) = world_dir(&state, &dir, &name, &world)?;
    Ok(state.store.list(&key).iter().map(view).collect())
}

#[tauri::command]
pub async fn world_snapshot(state: State<'_, AppState>, dir: String, name: String, world: String) -> CmdResult<Value> {
    let (wd, key) = world_dir(&state, &dir, &name, &world)?;
    let store = state.store.clone();
    let s = tauri::async_runtime::spawn_blocking(move || store.snapshot(&key, &wd, "这台电脑", "manual", KEEP)).await.map_err(|e| e.to_string())?.map_err(err)?;
    Ok(view(&s))
}

#[tauri::command]
pub async fn world_restore(state: State<'_, AppState>, dir: String, name: String, world: String, id: String) -> CmdResult<()> {
    let key_inst = format!("{dir}/{name}");
    if state.runs.lock().unwrap().values().any(|r| r.key == key_inst) {
        return Err("游戏还开着，先关掉再退回".into());
    }
    let (wd, key) = world_dir(&state, &dir, &name, &world)?;
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || store.restore(&key, &wd, &id, "这台电脑", KEEP)).await.map_err(|e| e.to_string())?.map_err(err)
}

/// 游戏退出后：这次运行期间保存过的世界各存一份（后台做，不挡界面）
pub fn after_run(store: std::sync::Arc<crate::sync::store::Store>, game_dir: PathBuf, since_ms: u64) {
    std::thread::spawn(move || {
        for e in std::fs::read_dir(game_dir.join("saves")).into_iter().flatten().flatten() {
            let level = e.path().join("level.dat");
            let changed = std::fs::metadata(&level).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).is_some_and(|d| d.as_millis() as u64 >= since_ms);
            if changed {
                let w = e.file_name().to_string_lossy().into_owned();
                if let Err(err) = store.snapshot(&world_key(&game_dir, &w), &e.path(), "这台电脑", "auto", KEEP) {
                    eprintln!("给「{w}」存历史失败：{err:#}");
                }
            }
        }
    });
}
