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
