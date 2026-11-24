//! LeafStarter 后端入口：只做启动时的准备和命令登记。
//!
//! 模块按 docs/ROADMAP.md 4.2 的边界拆：
//! - `base/`：基础设施（多源下载、任务、工具、平台桥接）
//! - `game/`：游戏文件、加载器、安装、Java、识别别的启动器
//! - `commands/`：给前端的命令，按页面分文件
//!
//! 以后的启动核心、账户、同步、联机、插件宿主各自成一个目录，插件只能通过它们公开的接口调用。

mod auth;
mod base;
mod cli;
mod commands;
mod game;
mod instance;
mod launch;
mod migrate;
mod state;

pub use cli::cli;

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

use base::net::Net;
use base::task::Tasks;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(not(target_os = "android"))]
    {
        builder = builder.plugin(tauri_plugin_dialog::init());
    }
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(base::platform::plugin());
    }
    builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("leafstarter"));
            std::fs::create_dir_all(&data_dir)?;
            // 第一次打开时把方块补给的设置搬过来（换了应用标识，数据目录也换了）
            let imported_from = migrate::import_mc_supply(&data_dir).map(|p| p.to_string_lossy().into_owned());
            let settings = state::load_settings(&data_dir);
            let net = Arc::new(Net::new(settings.net.clone())?);
            let data_dir_for_auth = data_dir.clone();
            app.manage(AppState {
                net: net.clone(),
                tasks: Tasks::default(),
                data_dir,
                settings: Mutex::new(settings),
                targets: Mutex::new(vec![]),
                manifest: tokio::sync::Mutex::new(None),
                imported_from,
                auth: Arc::new(auth::Auth::load(&data_dir_for_auth)),
                runs: Mutex::new(HashMap::new()),
                next_run: AtomicU64::new(0),
                logs: Mutex::new(HashMap::new()),
                msa_cancel: Mutex::new(None),
            });
            // 启动时后台测一次速，下载时就知道先用哪个源
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                net.speed_test().await;
                let _ = handle.emit("sources-updated", ());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::open_path,
            commands::versions::get_manifest,
            commands::versions::list_loaders,
            commands::targets::scan_targets,
            commands::targets::add_manual_dir,
            commands::targets::remove_target,
            commands::targets::pick_folder,
            commands::targets::android_status,
            commands::targets::android_request_all_files,
            commands::targets::android_grant,
            commands::settings::get_sources,
            commands::settings::test_sources,
            commands::settings::save_net_settings,
            commands::settings::list_java,
            commands::install::start_install,
            commands::instance::instance_files,
            commands::instance::set_mod_enabled,
            commands::instance::open_instance_dir,
            commands::instance::open_launcher,
            commands::launch::launch_instance,
            commands::launch::stop_instance,
            commands::launch::game_log,
            commands::launch::read_crash_report,
            commands::accounts::list_accounts,
            commands::accounts::offline_allowed,
            commands::accounts::add_offline_account,
            commands::accounts::remove_account,
            commands::accounts::set_default_account,
            commands::accounts::msa_begin,
            commands::accounts::msa_cancel,
            commands::tasks::cancel_task,
            commands::tasks::list_tasks,
            commands::tasks::clear_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
