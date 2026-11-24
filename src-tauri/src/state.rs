//! 全局状态：设置、任务、下载源、扫描到的游戏目录。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auth::Auth;
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
    /// 双击图标直接启动最近玩的实例（5.2）
    #[serde(default)]
    pub direct_launch: bool,
    /// 游戏窗口出来以后启动器怎么办：keep / min / hide
    #[serde(default)]
    pub after_launch: String,
    /// 最近启动的实例，新的在前，最多 10 个（跳转列表用前 5 个）
    #[serde(default)]
    pub recent: Vec<Recent>,
    /// 允许局域网里的设备连过来同步（5.7）
    #[serde(default)]
    pub lan_enabled: bool,
    /// AI 源、用途分配、MCP（5.13）
    #[serde(default)]
    pub ai: crate::agent::AiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    /// 版本文件夹，比如 D:/Games/.minecraft/versions/1.20.1-Forge
    pub path: String,
    pub name: String,
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
    pub auth: Arc<Auth>,
    /// 正在运行的游戏：run id → 控制
    pub runs: Mutex<HashMap<u64, RunCtl>>,
    pub next_run: AtomicU64,
    /// 每个实例最近一次运行的日志（键是界面给的实例 key）
    pub logs: Mutex<HashMap<String, Arc<Mutex<VecDeque<String>>>>>,
    /// 正在等的微软登录，取消用
    pub msa_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// 每个实例最近一次运行的最新状态（界面刷新或者后打开时补上）
    pub run_states: Mutex<HashMap<String, Value>>,
    pub direct: crate::direct::Direct,
    /// 正在二分查冲突的实例
    pub bisects: crate::commands::bisect::Running,
    /// 本机的文件仓库和存档历史
    pub store: Arc<crate::sync::store::Store>,
    /// 局域网直传：配对过的设备、服务端（开着「允许局域网连接」时才有）
    pub lan: Arc<crate::sync::lan::Ctx>,
    pub lan_server: Mutex<Option<crate::sync::lan::Server>>,
    pub agents: crate::agent::Agents,
    pub mcp_server: Mutex<Option<crate::agent::mcp::Server>>,
}

pub struct RunCtl {
    pub key: String,
    pub kill: Option<tokio::sync::oneshot::Sender<()>>,
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
