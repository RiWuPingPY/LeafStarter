//! 「版本」页：版本清单、加载器版本列表

use std::time::{Duration, Instant};

use tauri::State;

use crate::game::loaders::{self, LoaderKind};
use crate::game::mojang::{self, Manifest};
use crate::state::{err, AppState, CmdResult};

#[tauri::command]
pub async fn get_manifest(state: State<'_, AppState>, force: bool) -> CmdResult<Manifest> {
    let mut cache = state.manifest.lock().await;
    if let Some((at, m)) = cache.as_ref() {
        if !force && at.elapsed() < Duration::from_secs(600) {
            return Ok(m.clone());
        }
    }
    let m = mojang::fetch_manifest(&state.net).await.map_err(err)?;
    *cache = Some((Instant::now(), m.clone()));
    Ok(m)
}

#[tauri::command]
pub async fn list_loaders(state: State<'_, AppState>, kind: String, mc: String) -> CmdResult<Vec<loaders::LoaderVersion>> {
    let k = LoaderKind::parse(&kind).ok_or_else(|| format!("不支持的加载器：{}", kind))?;
    loaders::list(&state.net, k, &mc).await.map_err(err)
}
