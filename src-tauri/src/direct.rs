//! 直接启动（ROADMAP 5.2）：双击图标、桌面快捷方式、任务栏跳转列表 → 屏幕角落一个小窗倒数 3 秒 → 启动，
//! 主界面从头到尾不出现。打开时按住 Shift 就是正常界面。
//!
//! 主窗口一开始就建好、只是藏着（界面照常加载、照常收事件）。游戏正常退出后程序跟着退出；
//! 崩溃或者没能启动，就把主界面拿出来，停在那个实例上。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::{AppState, Recent, Settings};

const DELAY_MS: u64 = 3000;
const MINI_W: f64 = 360.0;
const MINI_H: f64 = 92.0;
const RECENT_KEEP: usize = 10;
const JUMP_LIST: usize = 5;

#[derive(Default)]
pub struct Direct {
    /// 直接启动模式：主界面还没出现过
    pub active: AtomicBool,
    /// 倒数时点了取消，或者又双击了一次图标
    pub cancel: AtomicBool,
    /// 游戏窗口出来后按设置把主界面藏起来了，游戏退出时要放回来
    pub hid_main: AtomicBool,
    /// 要告诉主界面的事（界面可能还没加载完，先存着，界面启动时来取）
    pub notice: Mutex<Option<Value>>,
}

/// 命令行里的 `--launch <版本文件夹>`
pub fn launch_arg(args: &[String]) -> Option<PathBuf> {
    let i = args.iter().position(|a| a == "--launch")?;
    args.get(i + 1).filter(|p| !p.is_empty()).map(PathBuf::from)
}

/// 打开时决定直接启动哪个实例；None 就是正常打开界面
pub fn decide(args: &[String], s: &Settings) -> Option<PathBuf> {
    if shift_down() {
        return None;
    }
    launch_arg(args).or_else(|| if s.direct_launch { s.recent.iter().find(|r| Path::new(&r.path).is_dir()).map(|r| PathBuf::from(&r.path)) } else { None })
}

fn shift_down() -> bool {
    #[cfg(windows)]
    {
        crate::base::shell::shift_down()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn show_main(app: &AppHandle) {
    let d = &app.state::<AppState>().direct;
    d.active.store(false, Relaxed);
    d.hid_main.store(false, Relaxed);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn close_mini(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.close();
    }
}

/// 告诉主界面：停到哪个实例上、提示什么
fn notice(app: &AppHandle, key: Option<&str>, msg: Option<String>) {
    let v = json!({ "key": key, "msg": msg });
    *app.state::<AppState>().direct.notice.lock().unwrap() = Some(v.clone());
    let _ = app.emit("direct-notice", v);
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) })
        .collect()
}

fn open_mini(app: &AppHandle, name: &str, loader: &str) -> tauri::Result<()> {
    close_mini(app);
    let url = format!("mini.html?n={}&l={}&d={}", enc(name), enc(loader), DELAY_MS);
    let mut b = WebviewWindowBuilder::new(app, "mini", WebviewUrl::App(url.into()))
        .title("LeafStarter")
        .inner_size(MINI_W, MINI_H)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(true);
    // WebView2 要求同一个程序里所有窗口的浏览器参数一样，不然建不出来（0x8007139F）：照抄主窗口的
    if let Some(args) = app.config().app.windows.iter().find(|w| w.label == "main").and_then(|w| w.additional_browser_args.clone()) {
        b = b.additional_browser_args(&args);
    }
    if let Ok(Some(m)) = app.primary_monitor() {
        let (wa, s) = (m.work_area(), m.scale_factor());
        let x = (wa.position.x as f64 + wa.size.width as f64) / s - MINI_W - 16.0;
        let y = (wa.position.y as f64 + wa.size.height as f64) / s - MINI_H - 16.0;
        b = b.position(x, y);
    }
    b.build().map(|_| ())
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// 重新扫一遍游戏目录，按版本文件夹找到实例：(游戏目录 id, 加载器)
async fn find(app: &AppHandle, version_dir: &Path) -> Option<(String, String, String)> {
    let name = version_dir.file_name()?.to_string_lossy().into_owned();
    let root = version_dir.parent().and_then(Path::parent)?.to_path_buf();
    let st = app.state::<AppState>();
    #[cfg(not(target_os = "android"))]
    {
        let manual = st.settings.lock().unwrap().manual_dirs.clone();
        let targets = tauri::async_runtime::spawn_blocking(move || crate::game::launchers::scan_desktop(&manual)).await.ok()?;
        *st.targets.lock().unwrap() = targets;
    }
    let targets = st.targets.lock().unwrap().clone();
    let t = targets.iter().find(|t| t.access == "direct" && norm(Path::new(&t.path)) == norm(&root))?;
    let v = t.versions.iter().find(|v| v.name.eq_ignore_ascii_case(&name))?;
    Some((t.id.clone(), v.name.clone(), v.loader.clone()))
}

/// 启动一个实例。`mini`：直接启动模式，角落小窗倒数；否则主界面已经在了，马上启动并停到那个实例上
pub fn begin(app: &AppHandle, version_dir: PathBuf, mini: bool) {
    let d = &app.state::<AppState>().direct;
    d.cancel.store(false, Relaxed);
    if mini {
        d.active.store(true, Relaxed);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        let Some((dir, name, loader)) = find(&app, &version_dir).await else {
            show_main(&app);
            return notice(&app, None, Some(format!("找不到要启动的实例：{}。可能改了名或者删掉了", version_dir.display())));
        };
        let key = format!("{}/{}", dir, name);
        if mini {
            if let Err(e) = open_mini(&app, &name, &loader) {
                eprintln!("小窗打不开：{e}");
            }
            while started.elapsed() < Duration::from_millis(DELAY_MS) {
                if app.state::<AppState>().direct.cancel.load(Relaxed) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if app.state::<AppState>().direct.cancel.load(Relaxed) {
                return;
            }
        } else {
            notice(&app, Some(&key), None);
        }
        let st = app.state::<AppState>();
        if let Err(e) = crate::commands::launch::launch_instance(app.clone(), st, dir, name, key.clone(), None, None, None).await {
            close_mini(&app);
            show_main(&app);
            notice(&app, Some(&key), Some(e));
        }
    });
}

/// 启动状态变了：小窗、主界面跟着变；直接启动的游戏正常退出后程序也退出
pub fn on_state(app: &AppHandle, key: &str, phase: &str) {
    let st = app.state::<AppState>();
    let d = &st.direct;
    match phase {
        "running" => {
            if d.active.load(Relaxed) {
                close_mini(app);
            } else if let Some(w) = app.get_webview_window("main") {
                match st.settings.lock().unwrap().after_launch.as_str() {
                    "min" => {
                        let _ = w.minimize();
                    }
                    "hide" => {
                        let _ = w.hide();
                        d.hid_main.store(true, Relaxed);
                    }
                    _ => {}
                }
            }
        }
        "exited" => {
            let others = st.runs.lock().unwrap().values().filter(|r| r.key != key).count();
            if d.active.load(Relaxed) {
                if others == 0 {
                    app.exit(0);
                }
            } else if d.hid_main.load(Relaxed) && others == 0 {
                show_main(app);
            }
        }
        "crashed" | "failed" => {
            close_mini(app);
            if d.active.load(Relaxed) || d.hid_main.load(Relaxed) {
                show_main(app);
            }
            notice(app, Some(key), None);
        }
        _ => {}
    }
}

/// 又打开了一次（双击图标、快捷方式、跳转列表）：参数交给已经在运行的这个
pub fn second_instance(app: &AppHandle, argv: Vec<String>) {
    let st = app.state::<AppState>();
    let want = if shift_down() { None } else { launch_arg(&argv) };
    let counting = app.get_webview_window("mini").is_some() && st.runs.lock().unwrap().is_empty();
    let main_visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    match want {
        // 什么都没在做，主界面也藏着：照直接启动的样子来
        Some(p) if !main_visible && !counting && st.runs.lock().unwrap().is_empty() => begin(app, p, true),
        Some(p) => {
            st.direct.cancel.store(true, Relaxed);
            close_mini(app);
            show_main(app);
            begin(app, p, false);
        }
        None => {
            st.direct.cancel.store(true, Relaxed);
            close_mini(app);
            show_main(app);
        }
    }
}

/// 记一笔最近启动，顺便更新任务栏跳转列表
pub fn remember(st: &AppState, root: &Path, name: &str) {
    let r = Recent { path: root.join("versions").join(name).to_string_lossy().into_owned(), name: name.to_string() };
    {
        let mut s = st.settings.lock().unwrap();
        s.recent.retain(|x| norm(Path::new(&x.path)) != norm(Path::new(&r.path)));
        s.recent.insert(0, r);
        s.recent.truncate(RECENT_KEEP);
    }
    st.save();
    refresh_jump_list(st);
}

/// 跳转列表：最近玩的 5 个（删掉、改了名的跳过）
pub fn refresh_jump_list(st: &AppState) {
    #[cfg(windows)]
    {
        let recent = st.settings.lock().unwrap().recent.clone();
        std::thread::spawn(move || {
            let Ok(exe) = std::env::current_exe() else { return };
            let items: Vec<_> = recent
                .iter()
                .filter(|r| Path::new(&r.path).is_dir())
                .take(JUMP_LIST)
                .map(|r| crate::base::shell::Item { title: r.name.clone(), args: format!("--launch \"{}\"", r.path), desc: format!("启动 {}", r.name) })
                .collect();
            if let Err(e) = crate::base::shell::set_jump_list(&exe, "最近玩的", &items) {
                eprintln!("跳转列表没更新上：{e:#}");
            }
        });
    }
    #[cfg(not(windows))]
    let _ = st;
}

/// 在桌面上建实例的快捷方式，返回建好的路径
pub fn desktop_shortcut(root: &Path, name: &str) -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    {
        let desk = dirs::desktop_dir().ok_or_else(|| anyhow::anyhow!("找不到桌面文件夹"))?;
        let lnk = desk.join(format!("{name}.lnk"));
        let exe = std::env::current_exe()?;
        let target = root.join("versions").join(name);
        crate::base::shell::create_shortcut(&lnk, &exe, &format!("--launch \"{}\"", target.display()), &format!("用 LeafStarter 启动 {name}"))?;
        Ok(lnk)
    }
    #[cfg(not(windows))]
    {
        let _ = (root, name);
        anyhow::bail!("这个系统还不支持桌面快捷方式")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(launch_arg(&a(&["ls.exe", "--launch", r"D:\mc\versions\1.20.1"])), Some(PathBuf::from(r"D:\mc\versions\1.20.1")));
        assert_eq!(launch_arg(&a(&["ls.exe"])), None);
        assert_eq!(launch_arg(&a(&["ls.exe", "--launch"])), None);
        assert_eq!(enc("空岛 1.20"), "%E7%A9%BA%E5%B2%9B%201.20");
        assert_eq!(norm(Path::new("D:/Games/.minecraft/")), norm(Path::new(r"d:\games\.minecraft")));
    }
}
