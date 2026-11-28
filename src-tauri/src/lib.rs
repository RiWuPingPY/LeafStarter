//! LeafStarter 后端入口：只做启动时的准备和命令登记。
//!
//! 模块按 docs/ROADMAP.md 4.2 的边界拆：
//! - `base/`：基础设施（多源下载、任务、工具、平台桥接）
//! - `game/`：游戏文件、加载器、安装、Java、识别别的启动器
//! - `commands/`：给前端的命令，按页面分文件
//!
//! 以后的启动核心、账户、同步、联机、插件宿主各自成一个目录，插件只能通过它们公开的接口调用。

mod agent;
mod auth;
mod base;
mod cli;
mod commands;
mod content;
mod direct;
mod game;
mod i18n;
mod instance;
mod launch;
mod migrate;
mod mp;
mod plugin;
mod server;
mod state;
mod sync;
#[cfg(not(target_os = "android"))]
mod tray;
mod update;

pub use cli::cli;
pub use update::pre_start;

/// main 一开始就调：启动计时从这里算
pub fn started() {
    direct::started();
}

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

use base::net::Net;
use base::task::Tasks;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 便携版：网页那边的数据（界面记住的东西）也放进旁边的 data/
    let portable = update::portable_data();
    if let Some(d) = &portable {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", d.join("webview"));
    }
    let mut builder = tauri::Builder::default();
    #[cfg(not(target_os = "android"))]
    {
        // 单实例要第一个注册：再打开一次（双击图标、快捷方式、跳转列表）时把参数交给已经在运行的这个
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| direct::second_instance(app, argv)))
            .plugin(tauri_plugin_dialog::init());
    }
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(base::platform::plugin());
    }
    builder
        .setup(move |app| {
            direct::mark("setup");
            if let Some(l) = mp::link_in(&std::env::args().collect::<Vec<_>>()) {
                mp::set_link(l);
            }
            let data_dir = portable.clone().unwrap_or_else(|| app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("leafstarter")));
            std::fs::create_dir_all(&data_dir)?;
            // 第一次打开时把方块补给的设置搬过来（换了应用标识，数据目录也换了）
            let imported_from = migrate::import_mc_supply(&data_dir).map(|p| p.to_string_lossy().into_owned());
            let settings = state::load_settings(&data_dir);
            let direct_target = if cfg!(target_os = "android") { None } else { direct::decide(&std::env::args().collect::<Vec<_>>(), &settings) };
            // 设置里填了代理：在建任何 HTTP 客户端之前设好，各处都按它走
            base::net::apply_proxy(&settings.net.proxy);
            let net = Arc::new(Net::new(settings.net.clone())?);
            let data_dir_for_auth = data_dir.clone();
            let store = Arc::new(sync::store::Store::open(&data_dir));
            let lan_on = settings.lan_enabled;
            let mcp_on = settings.ai.mcp;
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
                run_states: Mutex::new(HashMap::new()),
                direct: direct::Direct::default(),
                bisects: Mutex::new(HashMap::new()),
                store: store.clone(),
                lan: commands::lan::new_ctx(app.handle(), data_dir_for_auth.clone(), store.clone()),
                lan_server: Mutex::new(None),
                agents: agent::Agents::default(),
                #[cfg(windows)]
                safe_mode: base::shell::ctrl_down(),
                #[cfg(not(windows))]
                safe_mode: false,
                native: plugin::host::Native::default(),
                mcp_server: Mutex::new(None),
            });
            // 主窗口建好时是藏着的：直接启动就一直藏着；否则等界面画好第一屏（ui_ready）再拿出来，
            // 免得先露出白底和没上数据的骨架。界面脚本万一出错，3 秒后也拿出来
            match direct_target {
                Some(p) => direct::begin(app.handle(), p, true),
                None => {
                    let h = app.handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        direct::reveal(&h, None);
                    });
                }
            }
            direct::refresh_jump_list(&app.state::<AppState>());
            #[cfg(not(target_os = "android"))]
            if let Err(e) = tray::setup(app.handle()) {
                eprintln!("托盘图标没建起来：{e:#}");
            }
            if mcp_on {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = commands::ai::set_mcp(&h, true).await {
                        eprintln!("MCP 没开起来：{e:#}");
                    }
                });
            }
            if lan_on {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 先扫一遍游戏目录，别的设备来问实例时才有东西
                    let manual = h.state::<AppState>().settings.lock().unwrap().manual_dirs.clone();
                    #[cfg(not(target_os = "android"))]
                    if let Ok(t) = tauri::async_runtime::spawn_blocking(move || game::launchers::scan_desktop(&manual)).await {
                        *h.state::<AppState>().targets.lock().unwrap() = t;
                    }
                    if let Err(e) = commands::lan::set_enabled(&h, true).await {
                        eprintln!("局域网连接没开起来：{e:#}");
                    }
                });
            }
            // 自动更新：清掉换下来的旧版本，过一会儿看有没有新版本
            update::on_start(app.handle(), &app.state::<AppState>().data_dir);
            update::schedule(app.handle().clone());
            // 启动时后台测一次速，下载时就知道先用哪个源
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                net.speed_test().await;
                let _ = handle.emit("sources-updated", ());
            });
            direct::mark("setup_done");
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
            commands::instance::instance_config,
            commands::instance::set_instance_config,
            commands::instance::rename_instance,
            commands::instance::copy_instance,
            commands::instance::delete_instance,
            commands::instance::apply_fix,
            commands::instance::world_info,
            commands::instance::apply_options,
            commands::instance::instance_mods,
            commands::instance::watch_sig,
            commands::modgroups::mod_groups,
            commands::modgroups::mod_group_create,
            commands::modgroups::mod_group_switch,
            commands::modgroups::mod_group_rename,
            commands::modgroups::mod_group_delete,
            commands::launch::launch_instance,
            commands::launch::stop_instance,
            commands::launch::game_log,
            commands::launch::read_crash_report,
            commands::launch::run_states,
            commands::sync::world_history,
            commands::ai::ai_get,
            commands::ai::ai_save_source,
            commands::ai::ai_delete_source,
            commands::ai::ai_test,
            commands::ai::ai_set,
            commands::ai::agent_send,
            commands::ai::agent_answer,
            commands::ai::agent_undo,
            commands::ai::agent_reset,
            commands::ai::mcp_install,
            commands::i18n::i18n_scan,
            commands::mp::mp_hosts,
            commands::mp::mp_invite,
            commands::mp::mp_invite_read,
            commands::mp::mp_join_build,
            commands::mp::mp_join_group,
            commands::plugins::plugin_list,
            commands::plugins::plugin_inspect,
            commands::plugins::plugin_install,
            commands::plugins::plugin_pick,
            commands::plugins::plugin_enable,
            commands::plugins::plugin_remove,
            commands::plugins::plugin_index,
            commands::plugins::plugin_market_install,
            commands::plugins::plugin_market_inspect,
            commands::plugins::plugin_dev,
            commands::plugins::plugin_file,
            commands::plugins::plugin_fetch,
            commands::plugins::plugin_storage,
            commands::plugins::plugin_native,
            commands::edition::edition_get,
            commands::edition::edition_pick,
            commands::edition::edition_import,
            commands::edition::edition_remove,
            commands::edition::edition_build,
            commands::edition::edition_author,
            commands::edition::edition_pack,
            commands::edition::edition_update,
            commands::worldtools::world_map_regions,
            commands::worldtools::world_map_tile,
            commands::worldtools::world_structures,
            commands::worldtools::world_rules,
            commands::worldtools::world_edit,
            commands::skin::skin_profile,
            commands::skin::skin_fetch,
            commands::skin::skin_upload,
            commands::skin::skin_cape,
            commands::skin::skin_describe,
            commands::skin::skin_history,
            commands::skin::skin_history_save,
            commands::i18n::i18n_run,
            commands::i18n::i18n_remove,
            commands::server::server_list,
            commands::server::server_add_ssh,
            commands::server::server_set,
            commands::server::server_remove,
            commands::server::server_retrust,
            commands::server::server_harden,
            commands::server::server_setup,
            commands::server::server_power,
            commands::server::server_status,
            commands::server::server_cmd,
            commands::server::server_log,
            commands::server::server_backup,
            commands::server::server_backups,
            commands::server::server_world_up,
            commands::server::server_world_down,
            commands::server::server_firewall,
            commands::server::panel_probe,
            commands::server::panel_add,
            commands::server::panel_sync_mods,
            commands::content::content_search,
            commands::content::content_project,
            commands::content::cf_available,
            commands::content::content_versions,
            commands::content::content_plan,
            commands::content::content_install,
            commands::content::mod_updates,
            commands::content::perf_status,
            commands::content::perf_apply,
            commands::content::perf_remove,
            commands::content::pack_pick,
            commands::content::pack_read,
            commands::content::pack_install,
            commands::content::pack_install_modrinth,
            commands::content::pack_export,
            commands::lan::lan_status,
            commands::lan::lan_enable,
            commands::lan::lan_pair_code,
            commands::lan::lan_pair,
            commands::lan::lan_forget,
            commands::lan::lan_rename,
            commands::lan::lan_remote,
            commands::lan::lan_pull,
            commands::lan::lan_link,
            commands::lan::lan_sync_world,
            commands::sync::world_snapshot,
            commands::sync::world_restore,
            commands::bisect::bisect_estimate,
            commands::bisect::bisect_start,
            commands::bisect::bisect_cancel,
            commands::direct::get_launch_prefs,
            commands::direct::set_launch_prefs,
            commands::direct::create_shortcut,
            commands::direct::direct_cancel,
            commands::direct::direct_open,
            commands::direct::direct_notice,
            commands::direct::ui_ready,
            commands::mp::mp_p2p_join,
            commands::mp::mp_p2p_stop,
            commands::mp::mp_p2p_status,
            commands::mp::mp_nodes_get,
            commands::mp::mp_nodes_set,
            commands::plugins::plugin_tool_result,
            commands::content::pack_check,
            commands::content::pack_update,
            commands::app::ui_prefs_get,
            commands::app::ui_prefs_set,
            commands::targets::cached_targets,
            commands::accounts::list_accounts,
            commands::accounts::offline_allowed,
            commands::accounts::add_offline_account,
            commands::accounts::remove_account,
            commands::accounts::set_default_account,
            commands::accounts::msa_begin,
            commands::accounts::msa_cancel,
            commands::accounts::ygg_login,
            commands::accounts::ygg_pick,
            commands::tasks::cancel_task,
            commands::tasks::list_tasks,
            commands::tasks::clear_tasks,
            commands::mp::mp_take_link,
            commands::instance::mod_icons,
            commands::instance::mod_trash,
            commands::instance::latest_log,
            commands::content::gpu_list,
            commands::app::update_status,
            commands::app::update_check,
            commands::app::update_set,
            commands::app::update_restart,
        ])
        .build(tauri::generate_context!())
        .expect("启动失败")
        .run(|app, ev| {
            // 退出时把下好的新版本换上，下次打开就是新的
            if let tauri::RunEvent::Exit = ev {
                if let Err(e) = update::apply(&app.state::<AppState>().data_dir, false) {
                    eprintln!("新版本没换上：{e:#}");
                }
            }
        });
}
