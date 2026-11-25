//! 临时联机（M5）：本机地址、生成邀请码、看邀请码、照着邀请码建实例

use std::collections::HashSet;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

use crate::base::task;
use crate::content::{install, modrinth};
use crate::mp::{self, Addr, Invite};
use crate::state::{err, AppState, CmdResult};

/// 本机能给朋友的地址
#[tauri::command]
pub async fn mp_hosts() -> Vec<Value> {
    tauri::async_runtime::spawn_blocking(mp::hosts).await.unwrap_or_default().into_iter().map(|(k, h)| json!({ "kind": k, "host": h })).collect()
}

/// 生成邀请码：连接地址 + 这个实例的清单（Modrinth 上认得的 Mod 记版本 id，认不出的只记文件名）
#[tauri::command]
pub async fn mp_invite(state: State<'_, AppState>, dir: String, name: String, port: u16, extra: Option<String>) -> CmdResult<Value> {
    crate::instance::check_name(&name).map_err(err)?;
    let root = crate::commands::instance::root_of(&state, &dir)?;
    let gd = crate::instance::game_dir(&root, &name).0;
    let (mc, loader, lv) = {
        let t = state.targets.lock().unwrap();
        let v = t.iter().find(|t| t.id == dir).and_then(|t| t.versions.iter().find(|v| v.name == name)).ok_or("找不到这个实例")?;
        (v.mc.clone().ok_or("认不出这个实例的游戏版本")?, v.loader.to_lowercase(), v.loader_version.clone().unwrap_or_default())
    };
    let mut addrs: Vec<Addr> = tauri::async_runtime::spawn_blocking(mp::hosts).await.unwrap_or_default().into_iter().map(|(k, h)| Addr { k, h, p: port }).collect();
    // 朋友也可以直接连用户填的地址（樱花、自己的服务器……）
    if let Some(x) = extra.map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) {
        let (h, p) = match x.rsplit_once(':') {
            Some((h, p)) if p.parse::<u16>().is_ok() && !h.contains(':') => (h.to_string(), p.parse().unwrap()),
            _ => (x.clone(), 25565),
        };
        addrs.insert(0, Addr { k: "server".into(), h, p });
    }
    if addrs.is_empty() {
        return Err("这台电脑没有朋友能连的地址：没连局域网、没有公网 IPv6、也没开 Radmin。可以填一个樱花或服务器地址".into());
    }
    // Mod：按 SHA-1 去 Modrinth 认
    let mut list = vec![];
    for e in std::fs::read_dir(gd.join("mods")).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.to_lowercase().ends_with(".jar") && e.path().is_file() {
            list.push((n, crate::base::net::sha1_file(&e.path()).unwrap_or_default()));
        }
    }
    let http = modrinth::client().map_err(err)?;
    let found = modrinth::by_hashes(&http, &list.iter().map(|x| x.1.clone()).collect::<Vec<_>>()).await.unwrap_or_default();
    let mut mods = vec![];
    let mut miss = vec![];
    for (n, h) in list {
        match found.get(&h) {
            Some(v) => mods.push((v.project_id.clone(), v.id.clone(), n)),
            None => miss.push(n),
        }
    }
    let inv = Invite { v: 1, name: name.clone(), mc, loader, lv, mods, miss, addrs, at: crate::base::util::now_ms() };
    let code = mp::encode(&inv).map_err(err)?;
    Ok(json!({ "code": code, "link": format!("leafstarter://join/{code}"), "invite": inv }))
}

/// 看邀请码：内容、哪些地址连得上、本机有没有一样的实例
#[tauri::command]
pub async fn mp_invite_read(state: State<'_, AppState>, code: String) -> CmdResult<Value> {
    let inv = mp::decode(&code).map_err(err)?;
    let mut reach = vec![];
    for a in &inv.addrs {
        reach.push(json!({ "kind": a.k, "addr": a.text(), "ok": mp::reachable(a).await }));
    }
    // 本机已经有同版本、同加载器、Mod 也基本一样（九成以上的文件名对得上）的实例：可以直接进，同名的排前面
    let want: HashSet<&str> = inv.mods.iter().map(|m| m.2.as_str()).collect();
    let cands: Vec<(String, std::path::PathBuf, String)> = state
        .targets
        .lock()
        .unwrap()
        .iter()
        .filter(|t| t.access == "direct")
        .flat_map(|t| t.versions.iter().map(move |v| (t, v)))
        .filter(|(_, v)| v.mc.as_deref() == Some(inv.mc.as_str()) && v.loader.to_lowercase() == inv.loader)
        .map(|(t, v)| (format!("{}/{}", t.id, v.name), std::path::PathBuf::from(&t.path), v.name.clone()))
        .collect();
    let mut same: Vec<(bool, String)> = cands
        .into_iter()
        .filter(|(_, root, name)| {
            if want.is_empty() {
                return true;
            }
            let dir = crate::instance::game_dir(root, name).0.join("mods");
            let hit = std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| want.contains(e.file_name().to_string_lossy().as_ref())).count();
            hit * 10 >= want.len() * 9
        })
        .map(|(key, _, name)| (name != inv.name, key))
        .collect();
    same.sort();
    let same: Vec<String> = same.into_iter().map(|x| x.1).collect();
    let old_hours = (crate::base::util::now_ms().saturating_sub(inv.at)) / 3_600_000;
    Ok(json!({ "invite": inv, "reach": reach, "same": same, "hours": old_hours }))
}

/// 照着邀请码建实例：装游戏和加载器，再从 Modrinth 装 Mod
#[tauri::command]
pub async fn mp_join_build(app: AppHandle, state: State<'_, AppState>, code: String, dir: String, name: String) -> CmdResult<u64> {
    let inv = mp::decode(&code).map_err(err)?;
    crate::instance::check_name(&name).map_err(err)?;
    let root = crate::commands::instance::root_of(&state, &dir)?;
    if root.join("versions").join(&name).exists() {
        return Err(format!("已经有叫「{name}」的实例了，换个名字"));
    }
    let target = state.targets.lock().unwrap().iter().find(|t| t.id == dir).cloned().ok_or("找不到这个游戏目录")?;
    let task = state.tasks.create(format!("照邀请码建「{name}」"), format!("{} {}", inv.mc, inv.loader), format!("{} · {}", target.launcher, target.path));
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let (net, data_dir) = (state.net.clone(), state.data_dir.clone());
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            let loader = match crate::game::loaders::LoaderKind::parse(&inv.loader) {
                Some(k) if !inv.lv.is_empty() => {
                    task.stage(format!("找 {} {}", inv.loader, inv.lv));
                    let list = crate::game::loaders::list(&net, k, &inv.mc).await?;
                    let v = list.into_iter().find(|x| x.version == inv.lv).ok_or_else(|| anyhow::anyhow!("找不到 {} {}", inv.loader, inv.lv))?;
                    Some(crate::game::install::LoaderReq { kind: k.label().into(), id: v.id, version: v.version })
                }
                _ => None,
            };
            let env = crate::game::install::Env { app: Some(app.clone()), net: net.clone(), data_dir: data_dir.clone() };
            let req = crate::game::install::InstallRequest { target_id: target.id.clone(), mc_version: inv.mc.clone(), name: name.clone(), loader, overwrite: false };
            crate::game::install::run(&env, target.clone(), req, task.clone()).await?;
            let mut note = String::new();
            if !inv.mods.is_empty() {
                task.stage(format!("从 Modrinth 装 {} 个 Mod", inv.mods.len()));
                task.reset_counters();
                let http = modrinth::client()?;
                let ids: Vec<String> = inv.mods.iter().map(|m| m.1.clone()).collect::<HashSet<_>>().into_iter().collect();
                let mut items = vec![];
                for chunk in ids.chunks(100) {
                    for v in modrinth::versions_by_ids(&http, chunk).await? {
                        if let Some(f) = v.primary().cloned() {
                            items.push(install::Item { project: v.project_id.clone(), title: v.name.clone(), version: v.id.clone(), file: f, dep: false, replaces: None });
                        }
                    }
                }
                // 装进实例自己的 mods（建了这个文件夹，实例就是版本隔离的，不会混进公共 mods）
                let mods_dir = std::path::PathBuf::from(&target.path).join("versions").join(&name).join("mods");
                std::fs::create_dir_all(&mods_dir)?;
                install::apply(&net, &mods_dir, &items, &task.prog).await?;
                if items.len() < inv.mods.len() {
                    note = format!("；{} 个 Mod 在 Modrinth 上找不到了", inv.mods.len() - items.len());
                }
            }
            if !inv.miss.is_empty() {
                note.push_str(&format!("；这几个要找主人要：{}", inv.miss.join("、")));
            }
            Ok(Some(format!("建好了「{name}」{note}")))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}
