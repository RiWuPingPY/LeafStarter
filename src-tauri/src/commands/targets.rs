//! 「启动器」页：识别本机（安卓上是已装的）启动器和它们的游戏目录、手动添加目录、安卓目录授权

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::base::platform;
use crate::game::launchers::{self, SafGrant, Target};
use crate::state::{err, AppState, CmdResult};

async fn do_scan(app: &AppHandle, state: &AppState) -> Vec<Target> {
    let (manual, grants) = {
        let s = state.settings.lock().unwrap();
        (s.manual_dirs.clone(), s.grants.clone())
    };
    let targets = if cfg!(target_os = "android") {
        let apps = platform::installed_apps(app).await.unwrap_or_default();
        let all_files = platform::all_files_access(app).await;
        let mut saf_versions = HashMap::new();
        for g in &grants {
            saf_versions.insert(g.uri.clone(), platform::saf_versions(app, g).await);
        }
        tauri::async_runtime::spawn_blocking(move || launchers::scan_android(&apps, all_files, &grants, &saf_versions, &manual))
            .await
            .unwrap_or_default()
    } else {
        #[cfg(not(target_os = "android"))]
        {
            tauri::async_runtime::spawn_blocking(move || launchers::scan_desktop(&manual)).await.unwrap_or_default()
        }
        #[cfg(target_os = "android")]
        {
            vec![]
        }
    };
    *state.targets.lock().unwrap() = targets.clone();
    // 存一份：下次打开先用它画第一屏，扫描放到后台
    let _ = crate::base::util::write_json(&cache_file(state), &serde_json::to_value(&targets).unwrap_or_default());
    targets
}

fn cache_file(state: &AppState) -> PathBuf {
    state.data_dir.join("cache").join("targets.json")
}

/// 上次扫到的游戏目录和实例（打开时先用它画第一屏，`scan_targets` 在后台跟上）。
/// 后端这边也先装上，界面马上就能对这些实例做事
#[tauri::command]
pub fn cached_targets(state: State<'_, AppState>) -> Option<Vec<Target>> {
    let list: Vec<Target> = serde_json::from_slice(&std::fs::read(cache_file(&state)).ok()?).ok()?;
    let mut cur = state.targets.lock().unwrap();
    if cur.is_empty() {
        *cur = list.clone();
    }
    Some(list)
}

#[tauri::command]
pub async fn scan_targets(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<Target>> {
    Ok(do_scan(&app, &state).await)
}

#[tauri::command]
pub async fn add_manual_dir(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<Vec<Target>> {
    let p = PathBuf::from(path.trim());
    if !p.is_dir() {
        std::fs::create_dir_all(&p).map_err(|e| format!("无法使用这个目录：{}", e))?;
    }
    {
        let mut s = state.settings.lock().unwrap();
        let v = p.to_string_lossy().into_owned();
        if !s.manual_dirs.contains(&v) {
            s.manual_dirs.push(v);
        }
    }
    state.save();
    Ok(do_scan(&app, &state).await)
}

#[tauri::command]
pub async fn remove_target(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<Vec<Target>> {
    let target = state.targets.lock().unwrap().iter().find(|t| t.id == id).cloned();
    if let Some(t) = target {
        if let Some(uri) = &t.saf_uri {
            platform::release_tree(&app, uri).await;
        }
        let mut s = state.settings.lock().unwrap();
        s.manual_dirs.retain(|d| launchers::target_id(d) != id && !t.path.eq_ignore_ascii_case(d) && !t.path.starts_with(d.as_str()));
        if let Some(uri) = &t.saf_uri {
            s.grants.retain(|g| &g.uri != uri);
        }
    }
    state.save();
    Ok(do_scan(&app, &state).await)
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> CmdResult<Option<String>> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog().file().set_title("选择 .minecraft 文件夹").pick_folder(move |f| {
            let _ = tx.send(f.map(|p| p.to_string()));
        });
        Ok(rx.await.ok().flatten())
    }
    #[cfg(target_os = "android")]
    {
        let _ = app;
        Ok(None)
    }
}

#[tauri::command]
pub async fn android_status(app: AppHandle) -> CmdResult<Value> {
    Ok(json!({ "allFiles": platform::all_files_access(&app).await }))
}

#[tauri::command]
pub async fn android_request_all_files(app: AppHandle) -> CmdResult<()> {
    platform::request_all_files(&app).await.map_err(err)
}

/// 安卓：打开系统文件选择器授权一个目录（传包名时会尽量直接定位到那个启动器）
#[tauri::command]
pub async fn android_grant(app: AppHandle, state: State<'_, AppState>, package: Option<String>) -> CmdResult<Vec<Target>> {
    let picked = platform::pick_tree(&app, package.clone()).await.map_err(err)?;
    if picked.uri.is_empty() {
        return Err("没有选择目录".into());
    }
    if !picked.found {
        platform::release_tree(&app, &picked.uri).await;
        return Err("这个目录里没找到 .minecraft。请选启动器本身（侧边栏里的启动器名字），或者直接选 .minecraft 文件夹".into());
    }
    // 选的是不是我们认识的启动器：看授权地址里的包名
    let pkg = launchers::ANDROID_LAUNCHERS
        .iter()
        .map(|(p, _, _)| *p)
        .filter(|p| picked.uri.contains(&format!("{}.document.provider", p)) || picked.uri.contains(&format!("/{}", p)))
        .max_by_key(|p| p.len())
        .map(String::from)
        .or(package.filter(|p| picked.uri.contains(p.as_str())));
    {
        let mut s = state.settings.lock().unwrap();
        s.grants.retain(|g| g.uri != picked.uri && (pkg.is_none() || g.package != pkg));
        s.grants.push(SafGrant { uri: picked.uri.clone(), rel: picked.rel.clone(), package: pkg, label: picked.label.clone() });
    }
    state.save();
    Ok(do_scan(&app, &state).await)
}
