//! 局域网直传：开关、配对码和二维码、配对别的设备、看对方的实例、拉过来、同步世界。
//! 拉实例和同步世界的进度走「任务」页（和安装一样）。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::base::task;
use crate::commands::instance::root_of;
use crate::sync::devices::Devices;
use crate::sync::engine::{self, Link, Outcome};
use crate::sync::lan::{Ctx, Host, RemoteInst, RemoteWorld, Server};
use crate::sync::peer::{self, Client};
use crate::state::{err, AppState, CmdResult};

/// 真程序里的宿主：实例来自扫描结果
pub struct AppHost(pub AppHandle);

impl Host for AppHost {
    fn instances(&self) -> Vec<RemoteInst> {
        let st = self.0.state::<AppState>();
        let targets = st.targets.lock().unwrap().clone();
        let mut out = vec![];
        for t in targets.iter().filter(|t| t.access == "direct") {
            for v in &t.versions {
                let gd = crate::instance::game_dir(&PathBuf::from(&t.path), &v.name).0;
                let worlds = std::fs::read_dir(gd.join("saves"))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.path().join("level.dat").is_file())
                    .map(|e| RemoteWorld {
                        folder: e.file_name().to_string_lossy().into_owned(),
                        modified: e.path().join("level.dat").metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0),
                    })
                    .collect();
                out.push(RemoteInst { key: format!("{}/{}", t.id, v.name), name: v.name.clone(), mc: v.mc.clone(), loader: v.loader.clone(), loader_version: v.loader_version.clone(), launcher: t.launcher.clone(), worlds });
            }
        }
        out
    }

    fn game_dir(&self, key: &str) -> Option<PathBuf> {
        let (dir, name) = key.split_once('/')?;
        let st = self.0.state::<AppState>();
        let root = root_of(&st, dir).ok()?;
        crate::instance::check_name(name).ok()?;
        Some(crate::instance::game_dir(&root, name).0)
    }

    fn running(&self, key: &str) -> bool {
        self.0.state::<AppState>().runs.lock().unwrap().values().any(|r| r.key == key)
    }

    fn changed(&self, what: &str) {
        let _ = self.0.emit("lan-changed", what);
    }
}

/// 开发测试用：环境变量 LEAFSTARTER_LAN_LOOPBACK=1 时只听本机地址（自己和自己配对来测，不弹防火墙提示）
fn loopback_only() -> bool {
    std::env::var("LEAFSTARTER_LAN_LOOPBACK").is_ok_and(|v| v == "1")
}

fn status(st: &AppState) -> Value {
    let d = st.lan.devices.lock().unwrap().clone();
    let port = st.lan_server.lock().unwrap().as_ref().map(|s| s.port);
    json!({
        "enabled": port.is_some(),
        "port": port,
        "me": d.me,
        "ips": crate::sync::lan::local_ips(),
        "clients": d.clients.iter().map(|c| json!({ "id": c.id, "name": c.name, "added": c.added })).collect::<Vec<_>>(),
        "peers": d.peers.iter().map(|c| json!({ "id": c.id, "name": c.name, "added": c.added, "hosts": c.hosts })).collect::<Vec<_>>(),
    })
}

#[tauri::command]
pub fn lan_status(state: State<'_, AppState>) -> Value {
    status(&state)
}

/// 开着「允许局域网连接」就在后台听；设置里记下来，下次打开自动开
pub async fn set_enabled(app: &AppHandle, on: bool) -> anyhow::Result<()> {
    let st = app.state::<AppState>();
    let running = st.lan_server.lock().unwrap().is_some();
    if on && !running {
        let srv = Server::start(st.lan.clone(), if loopback_only() { "127.0.0.1" } else { "0.0.0.0" }.parse().unwrap()).await?;
        *st.lan_server.lock().unwrap() = Some(srv);
    } else if !on {
        st.lan_server.lock().unwrap().take();
    }
    st.settings.lock().unwrap().lan_enabled = on;
    st.save();
    Ok(())
}

#[tauri::command]
pub async fn lan_enable(app: AppHandle, on: bool) -> CmdResult<Value> {
    set_enabled(&app, on).await.map_err(err)?;
    Ok(status(&app.state::<AppState>()))
}

/// 配对码和二维码：会顺便打开「允许局域网连接」；10 分钟内有效，只能配一台
#[tauri::command]
pub async fn lan_pair_code(app: AppHandle) -> CmdResult<Value> {
    set_enabled(&app, true).await.map_err(err)?;
    let st = app.state::<AppState>();
    let port = st.lan_server.lock().unwrap().as_ref().map(|s| s.port).unwrap_or(crate::sync::lan::PORT);
    let token = st.lan.new_pair_token();
    let me = st.lan.devices.lock().unwrap().me.clone();
    let ips = if loopback_only() { vec!["127.0.0.1".to_string()] } else { crate::sync::lan::local_ips() };
    if ips.is_empty() {
        return Err("这台电脑没连着局域网（Wi-Fi 或网线）".into());
    }
    let code = peer::pair_code(&ips, port, &token, &me);
    let qr = qrcode::QrCode::new(code.as_bytes()).map_err(|e| e.to_string())?;
    let svg = qr.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).quiet_zone(true).build();
    Ok(json!({ "code": code, "svg": svg, "ips": ips, "port": port }))
}

#[tauri::command]
pub async fn lan_pair(state: State<'_, AppState>, code: String) -> CmdResult<Value> {
    let me = state.lan.devices.lock().unwrap().me.clone();
    let k = peer::pair(&code, &me).await.map_err(err)?;
    let name = k.name.clone();
    {
        let mut d = state.lan.devices.lock().unwrap();
        d.peers.retain(|p| p.id != k.id);
        d.peers.push(k);
        d.save(&state.data_dir).map_err(err)?;
    }
    Ok(json!({ "name": name, "status": status(&state) }))
}

#[tauri::command]
pub fn lan_forget(state: State<'_, AppState>, id: String) -> CmdResult<Value> {
    {
        let mut d = state.lan.devices.lock().unwrap();
        d.clients.retain(|c| c.id != id);
        d.peers.retain(|c| c.id != id);
        d.save(&state.data_dir).map_err(err)?;
    }
    Ok(status(&state))
}

#[tauri::command]
pub fn lan_rename(state: State<'_, AppState>, name: String) -> CmdResult<Value> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 30 {
        return Err("名字 1 到 30 个字".into());
    }
    {
        let mut d = state.lan.devices.lock().unwrap();
        d.me.name = n.to_string();
        d.save(&state.data_dir).map_err(err)?;
    }
    Ok(status(&state))
}

fn client(st: &AppState, peer_id: &str) -> CmdResult<Client> {
    let d: Devices = st.lan.devices.lock().unwrap().clone();
    let p = d.peer(peer_id).ok_or("找不到这台设备，重新配对一次")?;
    Client::new(&d.me, p).map_err(err)
}

/// 对方有哪些实例
#[tauri::command]
pub async fn lan_remote(state: State<'_, AppState>, peer: String) -> CmdResult<Value> {
    let c = client(&state, &peer)?;
    Ok(c.call("/info", json!({}), &[]).await.map_err(err)?.0)
}

/// 把对方的一个实例拉过来：本机没有这个版本就先装（从官方源下，不走对方），再拿 Mod、配置和全部世界
#[tauri::command]
pub async fn lan_pull(app: AppHandle, state: State<'_, AppState>, peer: String, remote: String, dir: String, name: String) -> CmdResult<u64> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    let c = client(&state, &peer)?;
    let (info, _) = c.call("/info", json!({}), &[]).await.map_err(err)?;
    let inst: RemoteInst = info["instances"].as_array().and_then(|a| a.iter().find(|i| i["key"] == remote.as_str())).and_then(|i| serde_json::from_value(i.clone()).ok()).ok_or("对方没有这个实例了")?;
    let peer_name = info["name"].as_str().unwrap_or("对方").to_string();
    let target = state.targets.lock().unwrap().iter().find(|t| t.id == dir).cloned().ok_or("找不到这个游戏目录")?;
    let task = state.tasks.create(format!("从「{peer_name}」拿「{}」", inst.name), name.clone(), format!("{} · {}", target.launcher, target.path));
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let (net, data_dir, store) = (state.net.clone(), state.data_dir.clone(), state.store.clone());
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            if !root.join("versions").join(&name).join(format!("{name}.json")).is_file() {
                let mc = inst.mc.clone().ok_or_else(|| anyhow::anyhow!("对方的实例认不出游戏版本，没法照着装"))?;
                let loader = match (crate::game::loaders::LoaderKind::parse(&inst.loader), &inst.loader_version) {
                    (Some(k), Some(ver)) => {
                        task.stage(format!("找 {} {}", inst.loader, ver));
                        let list = crate::game::loaders::list(&net, k, &mc).await?;
                        let v = list.into_iter().find(|x| &x.version == ver).ok_or_else(|| anyhow::anyhow!("找不到 {} {}", inst.loader, ver))?;
                        Some(crate::game::install::LoaderReq { kind: k.label().into(), id: v.id, version: v.version })
                    }
                    _ => None,
                };
                let env = crate::game::install::Env { app: Some(app.clone()), net: net.clone(), data_dir: data_dir.clone() };
                let req = crate::game::install::InstallRequest { target_id: target.id.clone(), mc_version: mc, name: name.clone(), loader, overwrite: false };
                crate::game::install::run(&env, target.clone(), req, task.clone()).await?;
            }
            // 装了加载器的实例默认版本隔离：Mod、配置、存档放在版本文件夹里
            let vdir = root.join("versions").join(&name);
            if !inst.loader.is_empty() {
                std::fs::create_dir_all(vdir.join("mods"))?;
            }
            let gd = crate::instance::game_dir(&root, &name).0;
            let t2 = task.clone();
            let prog = move |s: String| t2.stage(s);
            engine::pull_instance(&c, &store, &remote, &gd, &prog).await?;
            let mut link = Link { peer: peer.clone(), remote: remote.clone(), worlds: Default::default() };
            for w in &inst.worlds {
                prog(format!("拿世界「{}」", w.folder));
                let hk = crate::commands::sync::world_key(&gd, &w.folder);
                let (_, h) = engine::pull_world(&c, &store, &remote, &w.folder, &gd, None, true, &hk, &prog).await?;
                link.worlds.insert(w.folder.clone(), h);
            }
            let mut links = engine::load_links(&data_dir);
            links.insert(engine::link_key(&gd), link);
            engine::save_links(&data_dir, &links)?;
            Ok(Some(format!("拿好了：{} 个世界", inst.worlds.len())))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}

/// 这个实例和哪台设备同步着
#[tauri::command]
pub fn lan_link(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<Value> {
    let root = root_of(&state, &dir)?;
    let gd = crate::instance::game_dir(&root, &name).0;
    let links = engine::load_links(&state.data_dir);
    let Some(l) = links.get(&engine::link_key(&gd)) else { return Ok(Value::Null) };
    let peer = state.lan.devices.lock().unwrap().peer(&l.peer).map(|p| p.name.clone());
    Ok(json!({ "peer": l.peer, "peerName": peer, "remote": l.remote, "worlds": l.worlds }))
}

/// 同步一个世界：先看哪边变了；`prefer` 是冲突时用户选的那边（mine / theirs），平时为空
#[tauri::command]
pub async fn lan_sync_world(state: State<'_, AppState>, dir: String, name: String, world: String, prefer: Option<String>) -> CmdResult<Value> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&world).map_err(err)?;
    if state.runs.lock().unwrap().values().any(|r| r.key == format!("{dir}/{name}")) {
        return Err("游戏还开着，关掉再同步".into());
    }
    let gd = crate::instance::game_dir(&root, &name).0;
    let mut links = engine::load_links(&state.data_dir);
    let lk = engine::link_key(&gd);
    let l = links.get(&lk).cloned().ok_or("这个实例没有和别的设备同步过")?;
    let c = client(&state, &l.peer)?;
    let store = state.store.clone();
    let base = l.worlds.get(&world).cloned();
    let hk = crate::commands::sync::world_key(&gd, &world);
    let quiet = |_: String| {};
    let dir_w = gd.join("saves").join(&world);
    let mine_now = if dir_w.is_dir() { crate::sync::lan::files_hash(&store.scan(&dir_w).map_err(err)?) } else { String::new() };
    let local_changed = base.as_deref() != Some(mine_now.as_str()) && !mine_now.is_empty();
    let (out, hash, dirn) = match prefer.as_deref() {
        Some("theirs") => {
            let (o, h) = engine::pull_world(&c, &store, &l.remote, &world, &gd, base.as_deref(), true, &hk, &quiet).await.map_err(err)?;
            (o, h, "pull")
        }
        Some("mine") => {
            let (o, h) = engine::push_world(&c, &store, &l.remote, &world, &gd, base.as_deref(), true, &quiet).await.map_err(err)?;
            (o, h, "push")
        }
        _ if local_changed => {
            let (o, h) = engine::push_world(&c, &store, &l.remote, &world, &gd, base.as_deref(), false, &quiet).await.map_err(err)?;
            (o, h, "push")
        }
        _ => {
            let (o, h) = engine::pull_world(&c, &store, &l.remote, &world, &gd, base.as_deref(), false, &hk, &quiet).await.map_err(err)?;
            (o, h, "pull")
        }
    };
    if !matches!(out, Outcome::Conflict) {
        if let Some(e) = links.get_mut(&lk) {
            e.worlds.insert(world.clone(), hash);
        }
        engine::save_links(&state.data_dir, &links).map_err(err)?;
    }
    Ok(json!({ "outcome": out, "direction": dirn }))
}

pub fn new_ctx(app: &AppHandle, data_dir: PathBuf, store: Arc<crate::sync::store::Store>) -> Arc<Ctx> {
    Arc::new(Ctx::new(data_dir, store, Arc::new(AppHost(app.clone()))))
}
