//! 「设置」页：下载源（测速、偏好）和本机 Java

use serde::Serialize;
use tauri::State;

use crate::base::net::{NetSettings, SourceView};
use crate::game::java::{self, JavaInfo};
use crate::state::{AppState, CmdResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesView {
    settings: NetSettings,
    sources: Vec<SourceView>,
    best: String,
}

fn sources_view(state: &AppState) -> SourcesView {
    SourcesView { settings: state.net.settings(), sources: state.net.views(), best: state.net.best_source_id() }
}

#[tauri::command]
pub fn get_sources(state: State<'_, AppState>) -> SourcesView {
    sources_view(&state)
}

#[tauri::command]
pub async fn test_sources(state: State<'_, AppState>) -> CmdResult<SourcesView> {
    state.net.speed_test().await;
    Ok(sources_view(&state))
}

#[tauri::command]
pub fn save_net_settings(state: State<'_, AppState>, settings: NetSettings) -> SourcesView {
    state.net.set_settings(settings.clone());
    state.settings.lock().unwrap().net = settings;
    state.save();
    sources_view(&state)
}

#[tauri::command]
pub async fn list_java(state: State<'_, AppState>) -> CmdResult<Vec<JavaInfo>> {
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || java::detect(&dir)).await.map_err(|e| e.to_string())
}
