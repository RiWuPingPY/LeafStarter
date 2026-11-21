mod forge;
mod install;
mod java;
mod launchers;
mod loaders;
mod mojang;
mod net;
mod platform;
mod task;
mod util;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use install::{Env, InstallRequest};
use launchers::{SafGrant, Target};
use loaders::LoaderKind;
use mojang::Manifest;
use net::{Net, NetSettings};
use task::Tasks;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    net: NetSettings,
    #[serde(default)]
    manual_dirs: Vec<String>,
    #[serde(default)]
    grants: Vec<SafGrant>,
}

struct AppState {
    net: Arc<Net>,
    tasks: Tasks,
    data_dir: PathBuf,
    settings: Mutex<Settings>,
    targets: Mutex<Vec<Target>>,
    manifest: tokio::sync::Mutex<Option<(Instant, Manifest)>>,
}

impl AppState {
    fn save(&self) {
        let s = self.settings.lock().unwrap().clone();
        let _ = util::write_json(&self.data_dir.join("settings.json"), &serde_json::to_value(s).unwrap_or(Value::Null));
    }
}

type CmdResult<T> = Result<T, String>;

fn err(e: anyhow::Error) -> String {
    format!("{:#}", e)
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> Value {
    json!({
        "platform": if cfg!(target_os = "android") { "android" } else if cfg!(windows) { "windows" } else { std::env::consts::OS },
        "version": env!("CARGO_PKG_VERSION"),
        "dataDir": state.data_dir.to_string_lossy(),
    })
}

#[tauri::command]
async fn get_manifest(state: State<'_, AppState>, force: bool) -> CmdResult<Manifest> {
    let mut cache = state.manifest.lock().await;
    if let Some((at, m)) = cache.as_ref() {
        if !force && at.elapsed() < Duration::from_secs(600) {
            return Ok(m.clone());
        }
    }
    let m = mojang::fetch_manifest(&state.net).await.map_err(err)?;
    *cache = Some((Instant::now(), m.clone()));
    Ok(m)
}

#[tauri::command]
async fn list_loaders(state: State<'_, AppState>, kind: String, mc: String) -> CmdResult<Vec<loaders::LoaderVersion>> {
    let k = LoaderKind::parse(&kind).ok_or_else(|| format!("不支持的加载器：{}", kind))?;
    loaders::list(&state.net, k, &mc).await.map_err(err)
}

async fn do_scan(app: &AppHandle, state: &AppState) -> Vec<Target> {
    let (manual, grants) = {
        let s = state.settings.lock().unwrap();
        (s.manual_dirs.clone(), s.grants.clone())
    };
    let targets = if cfg!(target_os = "android") {
        let apps = platform::installed_apps(app).await.unwrap_or_default();
        let all_files = platform::all_files_access(app).await;
        let mut saf_versions = HashMap::new();
        for g in &grants {
            saf_versions.insert(g.uri.clone(), platform::saf_versions(app, g).await);
        }
        tauri::async_runtime::spawn_blocking(move || launchers::scan_android(&apps, all_files, &grants, &saf_versions, &manual))
            .await
            .unwrap_or_default()
    } else {
        #[cfg(not(target_os = "android"))]
        {
            tauri::async_runtime::spawn_blocking(move || launchers::scan_desktop(&manual)).await.unwrap_or_default()
        }
        #[cfg(target_os = "android")]
        {
            vec![]
        }
    };
    *state.targets.lock().unwrap() = targets.clone();
    targets
}

#[tauri::command]
async fn scan_targets(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<Target>> {
    Ok(do_scan(&app, &state).await)
}

#[tauri::command]
async fn add_manual_dir(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<Vec<Target>> {
    let p = PathBuf::from(path.trim());
    if !p.is_dir() {
        std::fs::create_dir_all(&p).map_err(|e| format!("无法使用这个目录：{}", e))?;
    }
    {
        let mut s = state.settings.lock().unwrap();
        let v = p.to_string_lossy().into_owned();
        if !s.manual_dirs.contains(&v) {
            s.manual_dirs.push(v);
        }
    }
    state.save();
    Ok(do_scan(&app, &state).await)
}

#[tauri::command]
async fn remove_target(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<Vec<Target>> {
    let target = state.targets.lock().unwrap().iter().find(|t| t.id == id).cloned();
    if let Some(t) = target {
        if let Some(uri) = &t.saf_uri {
            platform::release_tree(&app, uri).await;
        }
        let mut s = state.settings.lock().unwrap();
        s.manual_dirs.retain(|d| launchers::target_id(d) != id && !t.path.eq_ignore_ascii_case(d) && !t.path.starts_with(d.as_str()));
        if let Some(uri) = &t.saf_uri {
            s.grants.retain(|g| &g.uri != uri);
        }
    }
    state.save();
    Ok(do_scan(&app, &state).await)
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> CmdResult<Option<String>> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog().file().set_title("选择 .minecraft 文件夹").pick_folder(move |f| {
            let _ = tx.send(f.map(|p| p.to_string()));
        });
        Ok(rx.await.ok().flatten())
    }
    #[cfg(target_os = "android")]
    {
        let _ = app;
        Ok(None)
    }
}

#[tauri::command]
async fn android_status(app: AppHandle) -> CmdResult<Value> {
    Ok(json!({ "allFiles": platform::all_files_access(&app).await }))
}

#[tauri::command]
async fn android_request_all_files(app: AppHandle) -> CmdResult<()> {
    platform::request_all_files(&app).await.map_err(err)
}

/// 安卓：打开系统文件选择器授权一个目录（传包名时会尽量直接定位到那个启动器）
#[tauri::command]
async fn android_grant(app: AppHandle, state: State<'_, AppState>, package: Option<String>) -> CmdResult<Vec<Target>> {
    let picked = platform::pick_tree(&app, package.clone()).await.map_err(err)?;
    if picked.uri.is_empty() {
        return Err("没有选择目录".into());
    }
    if !picked.found {
        platform::release_tree(&app, &picked.uri).await;
        return Err("这个目录里没找到 .minecraft。请选启动器本身（侧边栏里的启动器名字），或者直接选 .minecraft 文件夹".into());
    }
    // 选的是不是我们认识的启动器：看授权地址里的包名
    let pkg = launchers::ANDROID_LAUNCHERS
        .iter()
        .map(|(p, _, _)| *p)
        .filter(|p| picked.uri.contains(&format!("{}.document.provider", p)) || picked.uri.contains(&format!("/{}", p)))
        .max_by_key(|p| p.len())
        .map(String::from)
        .or(package.filter(|p| picked.uri.contains(p.as_str())));
    {
        let mut s = state.settings.lock().unwrap();
        s.grants.retain(|g| g.uri != picked.uri && (pkg.is_none() || g.package != pkg));
        s.grants.push(SafGrant { uri: picked.uri.clone(), rel: picked.rel.clone(), package: pkg, label: picked.label.clone() });
    }
    state.save();
    Ok(do_scan(&app, &state).await)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourcesView {
    settings: NetSettings,
    sources: Vec<net::SourceView>,
    best: String,
}

fn sources_view(state: &AppState) -> SourcesView {
    SourcesView { settings: state.net.settings(), sources: state.net.views(), best: state.net.best_source_id() }
}

#[tauri::command]
fn get_sources(state: State<'_, AppState>) -> SourcesView {
    sources_view(&state)
}

#[tauri::command]
async fn test_sources(state: State<'_, AppState>) -> CmdResult<SourcesView> {
    state.net.speed_test().await;
    Ok(sources_view(&state))
}

#[tauri::command]
fn save_net_settings(state: State<'_, AppState>, settings: NetSettings) -> SourcesView {
    state.net.set_settings(settings.clone());
    state.settings.lock().unwrap().net = settings;
    state.save();
    sources_view(&state)
}

#[tauri::command]
async fn start_install(app: AppHandle, state: State<'_, AppState>, req: InstallRequest) -> CmdResult<u64> {
    let target = state
        .targets
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == req.target_id)
        .cloned()
        .ok_or("找不到目标目录，请重新扫描启动器")?;
    let title = match &req.loader {
        Some(l) => format!("{} · {} {}", req.mc_version, l.kind, l.version),
        None => req.mc_version.clone(),
    };
    let task = state.tasks.create(title, req.name.clone(), format!("{} · {}", target.launcher, target.path));
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let env = Env { app: Some(app.clone()), net: state.net.clone(), data_dir: state.data_dir.clone() };
    tauri::async_runtime::spawn(async move {
        let result = install::run(&env, target, req, task.clone()).await;
        task::finish(&app, &task, result);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}

/// 调试 / 高级用法：`mc-supply --cli <命令>`，不开窗口直接跑。
/// 返回 None 表示不是命令行模式。
pub fn cli(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--cli") {
        return None;
    }
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().ok()?;
    let code = rt.block_on(async {
        let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
        let net = Arc::new(Net::new(NetSettings::default()).expect("http client"));
        let data_dir = std::env::temp_dir().join("mc-supply-cli");
        match args.get(2).map(String::as_str) {
            Some("sources") => {
                net.speed_test().await;
                println!("{}", serde_json::to_string_pretty(&net.views()).unwrap());
                0
            }
            Some("scan") => {
                #[cfg(not(target_os = "android"))]
                for t in launchers::scan_desktop(&[]) {
                    println!("{:<14} {:<4} {}  ({} 个版本)", t.launcher, t.access, t.path, t.versions.len());
                }
                0
            }
            Some("java") => {
                for j in java::detect(&data_dir) {
                    println!("Java {:<3} {:<12} {}  [{}]", j.major, j.version, j.path, j.source);
                }
                0
            }
            Some("loaders") => {
                let kind = get("--loader").and_then(|k| LoaderKind::parse(&k)).unwrap_or(LoaderKind::Fabric);
                let mc = get("--mc").unwrap_or_else(|| "1.20.1".into());
                match loaders::list(&net, kind, &mc).await {
                    Ok(list) => {
                        for v in list.iter().take(15) {
                            println!("{:<16} {:<6} {}", v.version, v.tag.clone().unwrap_or_default(), v.id);
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("{:#}", e);
                        1
                    }
                }
            }
            Some("install") => {
                let dir = get("--dir").expect("--dir <.minecraft 目录>");
                let mc = get("--mc").expect("--mc <版本>");
                net.speed_test().await;
                let loader = match (get("--loader"), get("--loader-version")) {
                    (Some(kind), Some(ver)) => {
                        let k = LoaderKind::parse(&kind).expect("loader");
                        let list = loaders::list(&net, k, &mc).await.expect("loader list");
                        let v = list.into_iter().find(|v| v.version == ver).expect("找不到这个加载器版本");
                        Some(install::LoaderReq { kind: k.label().into(), id: v.id, version: v.version })
                    }
                    _ => None,
                };
                let name = get("--name").unwrap_or_else(|| match &loader {
                    Some(l) => format!("{}-{} {}", mc, l.kind, l.version),
                    None => mc.clone(),
                });
                let target = Target {
                    id: launchers::target_id(&dir),
                    kind: "custom".into(),
                    launcher: "命令行".into(),
                    path: dir.clone(),
                    access: "direct".into(),
                    exe: None,
                    versions: vec![],
                    manual: true,
                    saf_uri: None,
                    saf_rel: None,
                    package: None,
                    hint: None,
                };
                let tasks = Tasks::default();
                let task = tasks.create(name.clone(), String::new(), dir.clone());
                let env = Env { app: None, net: net.clone(), data_dir };
                let req = InstallRequest { target_id: target.id.clone(), mc_version: mc, name, loader, overwrite: true };
                let t2 = task.clone();
                let ticker = tokio::spawn(async move {
                    let mut last = String::new();
                    loop {
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        let s = t2.snapshot();
                        let line = format!("{} [{}/{} 文件 {}]", s.stage, s.files_done, s.files_total, util::human_size(s.bytes_done));
                        if line != last {
                            println!("{}", line);
                            last = line;
                        }
                    }
                });
                let t0 = Instant::now();
                let r = install::run(&env, target, req, task.clone()).await;
                ticker.abort();
                let s = task.snapshot();
                println!("各源下载量：{:?}", s.sources.iter().map(|(k, v)| format!("{}={}", k, util::human_size(*v))).collect::<Vec<_>>());
                match r {
                    Ok(p) => {
                        println!("完成，用时 {:.1}s：{}", t0.elapsed().as_secs_f64(), p.unwrap_or_default());
                        if let Some(n) = s.note {
                            println!("{}", n);
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("失败：{:#}", e);
                        1
                    }
                }
            }
            _ => {
                eprintln!("用法：mc-supply --cli <sources|scan|java|loaders|install> [--mc 版本] [--loader 类型] [--loader-version 版本] [--dir 目录]");
                2
            }
        }
    });
    Some(code)
}

#[tauri::command]
fn cancel_task(state: State<'_, AppState>, id: u64) {
    if let Some(t) = state.tasks.get(id) {
        t.prog.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> Vec<task::TaskSnapshot> {
    state.tasks.all()
}

#[tauri::command]
fn clear_tasks(state: State<'_, AppState>) -> Vec<task::TaskSnapshot> {
    state.tasks.clear_finished();
    state.tasks.all()
}

#[tauri::command]
async fn list_java(state: State<'_, AppState>) -> CmdResult<Vec<java::JavaInfo>> {
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || java::detect(&dir)).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn open_path(path: String) -> CmdResult<()> {
    platform::open_path(&path).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(not(target_os = "android"))]
    {
        builder = builder.plugin(tauri_plugin_dialog::init());
    }
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(platform::plugin());
    }
    builder
        .setup(|app| {
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("mc-supply"));
            std::fs::create_dir_all(&data_dir)?;
            let settings: Settings = util::read_json(&data_dir.join("settings.json"))
                .ok()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            let net = Arc::new(Net::new(settings.net.clone())?);
            app.manage(AppState {
                net: net.clone(),
                tasks: Tasks::default(),
                data_dir,
                settings: Mutex::new(settings),
                targets: Mutex::new(vec![]),
                manifest: tokio::sync::Mutex::new(None),
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
            app_info,
            get_manifest,
            list_loaders,
            scan_targets,
            add_manual_dir,
            remove_target,
            pick_folder,
            android_status,
            android_request_all_files,
            android_grant,
            get_sources,
            test_sources,
            save_net_settings,
            start_install,
            cancel_task,
            list_tasks,
            clear_tasks,
            list_java,
            open_path,
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
