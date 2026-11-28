//! 系统托盘：左键拿出主界面；右键菜单能直接启动最近玩的实例、打开启动器、退出。
//! 游戏启动后按设置把主界面藏起来时，从这里拿回来。
//! 菜单在鼠标移到图标上时按「最近玩的」重新生成，不用到处通知它

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::direct;
use crate::state::AppState;

const RECENT: usize = 5;

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let recent = app.state::<AppState>().settings.lock().unwrap().recent.clone();
    let menu = Menu::new(app)?;
    let mut any = false;
    for (k, r) in recent.iter().filter(|r| std::path::Path::new(&r.path).is_dir()).take(RECENT).enumerate() {
        menu.append(&MenuItem::with_id(app, format!("launch:{k}"), format!("启动「{}」", r.name), true, None::<&str>)?)?;
        any = true;
    }
    if any {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(app, "open", "打开 LeafStarter", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?)?;
    Ok(menu)
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let mut b = TrayIconBuilder::with_id("main")
        .tooltip("LeafStarter")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| {
            let id = ev.id().as_ref();
            if id == "open" {
                direct::show_main(app);
            } else if id == "quit" {
                app.exit(0);
            } else if let Some(k) = id.strip_prefix("launch:").and_then(|k| k.parse::<usize>().ok()) {
                let recent = app.state::<AppState>().settings.lock().unwrap().recent.clone();
                if let Some(r) = recent.iter().filter(|r| std::path::Path::new(&r.path).is_dir()).nth(k) {
                    // 和双击快捷方式一样：主界面藏着就照直接启动来（角落小窗倒数），开着就在主界面里启动
                    direct::second_instance(app, vec![String::new(), "--launch".into(), r.path.clone()]);
                }
            }
        })
        .on_tray_icon_event(|tray, ev| match ev {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => direct::show_main(tray.app_handle()),
            TrayIconEvent::Enter { .. } => {
                if let Ok(m) = build_menu(tray.app_handle()) {
                    let _ = tray.set_menu(Some(m));
                }
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}
