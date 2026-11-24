//! 全局状态：设置、任务、下载源、扫描到的游戏目录。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::base::net::{Net, NetSettings};
use crate::base::task::Tasks;
use crate::base::util;
use crate::game::launchers::{SafGrant, Target};
use crate::game::mojang::Manifest;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub net: NetSettings,
    #[serde(default)]
    pub manual_dirs: Vec<String>,
    #[serde(default)]
    pub grants: Vec<SafGrant>,
}

pub struct AppState {
    pub net: Arc<Net>,
    pub tasks: Tasks,
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub targets: Mutex<Vec<Target>>,
    pub manifest: tokio::sync::Mutex<Option<(Instant, Manifest)>>,
    /// 这次启动从方块补给搬过设置时，记下从哪搬的（只有搬的那一次有值）
    pub imported_from: Option<String>,
}

impl AppState {
    pub fn save(&self) {
        let s = self.settings.lock().unwrap().clone();
        let _ = util::write_json(&self.data_dir.join("settings.json"), &serde_json::to_value(s).unwrap_or(Value::Null));
    }
}

/// 读设置；文件不存在或坏了就用默认值
pub fn load_settings(data_dir: &Path) -> Settings {
    util::read_json(&data_dir.join("settings.json"))
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub type CmdResult<T> = Result<T, String>;

pub fn err(e: anyhow::Error) -> String {
    format!("{:#}", e)
}
