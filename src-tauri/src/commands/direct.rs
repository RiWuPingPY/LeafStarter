//! 直接启动、桌面快捷方式、启动后启动器怎么办（ROADMAP 5.2）

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::commands::instance::root_of;
use crate::direct;
use crate::state::{err, AppState, CmdResult, Recent};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPrefs {
    pub direct_launch: bool,
    pub after_launch: String,
    #[serde(default)]
    pub recent: Vec<Recent>,
}

#[tauri::command]
pub fn get_launch_prefs(state: State<'_, AppState>) -> LaunchPrefs {
    let s = state.settings.lock().unwrap();
    let after = if s.after_launch.is_empty() { "keep".into() } else { s.after_launch.clone() };
    LaunchPrefs { direct_launch: s.direct_launch, after_launch: after, recent: s.recent.iter().filter(|r| std::path::Path::new(&r.path).is_dir()).cloned().collect() }
}

#[tauri::command]
pub fn set_launch_prefs(state: State<'_, AppState>, prefs: LaunchPrefs) -> CmdResult<()> {
    if !["keep", "min", "hide"].contains(&prefs.after_launch.as_str()) {
        return Err("不认识的选项".into());
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.direct_launch = prefs.direct_launch;
        s.after_launch = prefs.after_launch;
    }
    state.save();
    Ok(())
}

#[tauri::command]
pub fn create_shortcut(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<String> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    direct::desktop_shortcut(&root, &name).map(|p| p.to_string_lossy().into_owned()).map_err(err)
}

/// 小窗上的「取消」：还没启动就整个退出；已经在启动了只关小窗
#[tauri::command]
pub fn direct_cancel(app: AppHandle) {
    let st = app.state::<AppState>();
    st.direct.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    let idle = st.runs.lock().unwrap().is_empty();
    if idle && st.direct.active.load(std::sync::atomic::Ordering::Relaxed) {
        app.exit(0);
    } else if let Some(w) = app.get_webview_window("mini") {
        let _ = w.close();
    }
}

/// 小窗上的「打开启动器」：倒数中就不启动了，已经在启动就接着启动
#[tauri::command]
pub fn direct_open(app: AppHandle) {
    let st = app.state::<AppState>();
    st.direct.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.close();
    }
    direct::show_main(&app);
}

/// 主界面加载完来取：直接启动出了什么事、要停到哪个实例上
#[tauri::command]
pub fn direct_notice(state: State<'_, AppState>) -> Option<serde_json::Value> {
    state.direct.notice.lock().unwrap().take()
}
