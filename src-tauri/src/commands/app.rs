//! 应用信息、打开文件夹

use serde_json::{json, Value};
use tauri::State;

use crate::base::platform;
use crate::state::{err, AppState, CmdResult};

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> Value {
    json!({
        "platform": if cfg!(target_os = "android") { "android" } else if cfg!(windows) { "windows" } else { std::env::consts::OS },
        "version": env!("CARGO_PKG_VERSION"),
        "dataDir": state.data_dir.to_string_lossy(),
        "importedFrom": state.imported_from,
        "totalMemoryMb": crate::launch::jvm::total_memory_mb(),
        "autoMemoryMb": crate::launch::jvm::auto_memory_mb(crate::launch::jvm::total_memory_mb()),
    })
}

/// 用系统打开：文件夹、网页（http / https）、日志和图片这类不会运行的文件。
/// 不接受 exe、bat 这类能运行的东西：界面上显示的内容里万一混进了奇怪的路径，也不会被拿来执行
#[tauri::command]
pub fn open_path(path: String) -> CmdResult<()> {
    let p = std::path::Path::new(&path);
    let lower = path.to_ascii_lowercase();
    let url = lower.starts_with("https://") || lower.starts_with("http://");
    const SAFE: &[&str] = &["txt", "log", "json", "png", "jpg", "jpeg", "toml", "cfg", "properties", "md"];
    let safe_file = p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| SAFE.contains(&e.to_ascii_lowercase().as_str()));
    if !(url || p.is_dir() || safe_file) {
        return Err("只能打开文件夹、网页和日志、图片这类文件".into());
    }
    if url && path.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err("网址不对".into());
    }
    platform::open_path(&path).map_err(err)
}

// ---------- 自动更新（M6） ----------

/// 更新状态 + 设置；刚升级完的那一句看过就清
#[tauri::command]
pub fn update_status(state: State<'_, AppState>) -> Value {
    let st = crate::update::status();
    let s = state.settings.lock().unwrap();
    let v = json!({
        "version": crate::update::VERSION,
        "auto": !s.update_off,
        "channel": if s.update_channel.is_empty() { "stable" } else { s.update_channel.as_str() },
        "portable": crate::update::portable_data().is_some(),
        "status": st,
    });
    drop(s);
    if st.updated_from.is_some() {
        crate::update::clear_updated();
    }
    v
}

/// 现在就看一下有没有新版本（有就下好）
#[tauri::command]
pub async fn update_check(app: tauri::AppHandle) -> CmdResult<()> {
    crate::update::check(app, true).await.map_err(err)
}

#[tauri::command]
pub fn update_set(state: State<'_, AppState>, auto: bool, channel: String) -> CmdResult<()> {
    if !matches!(channel.as_str(), "stable" | "beta") {
        return Err("没有这个频道".into());
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.update_off = !auto;
        s.update_channel = channel;
    }
    state.save();
    Ok(())
}

/// 现在重启换上新版本（游戏开着的时候不行：游戏跟着启动器）
#[tauri::command]
pub fn update_restart(app: tauri::AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if !state.runs.lock().unwrap().is_empty() {
        return Err("游戏还开着，关掉游戏再重启；或者直接关掉启动器，下次打开就是新版本".into());
    }
    match crate::update::apply(&state.data_dir, true).map_err(err)? {
        true => {
            app.exit(0);
            Ok(())
        }
        false => Err("没有下好的新版本".into()),
    }
}
