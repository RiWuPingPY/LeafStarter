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

#[tauri::command]
pub fn open_path(path: String) -> CmdResult<()> {
    platform::open_path(&path).map_err(err)
}
