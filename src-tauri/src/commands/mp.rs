//! 临时联机（M5）：本机地址、生成邀请码、看邀请码、照着邀请码建实例

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::base::net::{FileJob, Net};
use crate::base::task::{self, TaskEntry};
use crate::content::{curseforge, install, modrinth};
use crate::game::modgroups;
use crate::mp::{self, Addr, Invite};
use crate::state::{err, AppState, CmdResult};

/// 本机能给朋友的地址
#[tauri::command]
pub async fn mp_hosts() -> Vec<Value> {
    tauri::async_runtime::spawn_blocking(mp::hosts).await.unwrap_or_default().into_iter().map(|(k, h)| json!({ "kind": k, "host": h })).collect()
}

/// 点邀请链接打开启动器时带进来的链接（取一次就清）
#[tauri::command]
pub fn mp_take_link() -> Option<String> {
    mp::take_link()
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
    // Modrinth 上没有的再去 CurseForge 按指纹认（比如只在 CurseForge 发 Fabric 版的 Mod）
    let mut cf = vec![];
    if !miss.is_empty() {
        let fps: Vec<(String, u32)> = miss.iter().filter_map(|n| std::fs::read(gd.join("mods").join(n)).ok().map(|b| (n.clone(), curseforge::fingerprint(&b)))).collect();
        if let Ok(found) = curseforge::files_by_fingerprints(&fps.iter().map(|x| x.1).collect::<Vec<_>>()).await {
            for (n, fp) in fps {
                if let Some((_, fid)) = found.get(&fp) {
                    cf.push((*fid, n.clone()));
                    miss.retain(|m| *m != n);
                }
            }
        }
    }
    let inv = Invite { v: 1, name: name.clone(), mc, loader, lv, mods, miss, cf, addrs, at: crate::base::util::now_ms() };
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
    let want: HashSet<&str> = inv.mods.iter().map(|m| m.2.as_str()).chain(inv.cf.iter().map(|m| m.1.as_str())).collect();
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
    // 版本一样、Mod 不一样的（开了版本隔离才行）：别的组里正好有一套 → 切过去；没有 → 在里面新建一组，不用重下游戏
    let mut groups = vec![];
    let mut fit = vec![];
    if !want.is_empty() {
        let list: Vec<(String, PathBuf, String)> = state
            .targets
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.access == "direct")
            .flat_map(|t| t.versions.iter().map(move |v| (t, v)))
            .filter(|(_, v)| v.mc.as_deref() == Some(inv.mc.as_str()) && v.loader.to_lowercase() == inv.loader)
            .map(|(t, v)| (format!("{}/{}", t.id, v.name), PathBuf::from(&t.path), v.name.clone()))
            .collect();
        for (key, root, name) in list {
            let (gd, isolated) = crate::instance::game_dir(&root, &name);
            if !isolated || same.contains(&key) {
                continue;
            }
            let g = modgroups::load(&gd);
            let mut all: HashSet<String> = HashSet::new();
            for x in &g.groups {
                let files = modgroups::files(&modgroups::dir_of(&gd, &g, &x.id));
                let hit = files.iter().filter(|f| want.contains(f.as_str())).count();
                if x.id != g.active && hit * 10 >= want.len() * 9 {
                    groups.push(json!({ "key": key, "name": name, "group": x.id, "groupName": x.name }));
                }
                all.extend(files);
            }
            let have = want.iter().filter(|f| all.contains(**f)).count();
            fit.push(json!({ "key": key, "name": name, "have": have, "need": want.len() - have, "same": name == inv.name }));
        }
        fit.sort_by_key(|x| (!x["same"].as_bool().unwrap_or(false), x["need"].as_u64().unwrap_or(0)));
    }
    let old_hours = (crate::base::util::now_ms().saturating_sub(inv.at)) / 3_600_000;
    Ok(json!({ "invite": inv, "reach": reach, "same": same, "groups": groups, "fit": fit, "hours": old_hours }))
}

/// 照邀请码把 Mod 下到一个文件夹：Modrinth 的、CurseForge 的；`have` 里（本机别的组）已经有的直接链过来。返回要告诉玩家的话
async fn fetch_mods(net: &Arc<Net>, dir: &Path, inv: &Invite, task: &Arc<TaskEntry>, have: &HashMap<String, PathBuf>) -> anyhow::Result<String> {
    std::fs::create_dir_all(dir)?;
    let mut note = String::new();
    let mut jobs: Vec<FileJob> = vec![];
    if !inv.mods.is_empty() {
        task.stage(format!("查 {} 个 Mod", inv.mods.len() + inv.cf.len()));
        let http = modrinth::client()?;
        let ids: Vec<String> = inv.mods.iter().map(|m| m.1.clone()).collect::<HashSet<_>>().into_iter().collect();
        let mut got = 0;
        for chunk in ids.chunks(100) {
            for v in modrinth::versions_by_ids(&http, chunk).await? {
                if let Some(f) = v.primary() {
                    got += 1;
                    jobs.push(FileJob { url: f.url.clone(), path: dir.join(install::safe_name(&f.filename)?), sha1: Some(f.sha1.clone()).filter(|s| !s.is_empty()), size: Some(f.size) });
                }
            }
        }
        if got < ids.len() {
            note.push_str(&format!("；{} 个 Mod 在 Modrinth 上找不到了", ids.len() - got));
        }
    }
    if !inv.cf.is_empty() {
        let mut blocked = vec![];
        for (_, name, url, sha1, size) in curseforge::files(&inv.cf.iter().map(|x| x.0).collect::<Vec<_>>()).await? {
            if url.is_empty() {
                blocked.push(name);
            } else {
                jobs.push(FileJob { url, path: dir.join(install::safe_name(&name)?), sha1, size });
            }
        }
        if !blocked.is_empty() {
            note.push_str(&format!("；这几个作者不让第三方下载，要找主人要：{}", blocked.join("、")));
        }
    }
    // 本机已经有的（别的组里）链过来，不占空间也不用下
    let mut linked = 0;
    for j in &jobs {
        if let Some(p) = j.sha1.as_ref().and_then(|h| have.get(&h.to_lowercase())) {
            if modgroups::link_or_copy(p, &j.path).is_ok() {
                linked += 1;
            }
        }
    }
    task.stage(if linked > 0 { format!("下载 {} 个 Mod（{} 个本机已有）", jobs.len() - linked, linked) } else { format!("下载 {} 个 Mod", jobs.len()) });
    task.reset_counters();
    crate::game::install::download_all(net, &jobs, &task.prog).await?;
    if !inv.miss.is_empty() {
        note.push_str(&format!("；这几个要找主人要：{}", inv.miss.join("、")));
    }
    Ok(note)
}

/// 版本一样、Mod 不一样：在已有的实例里新建一组朋友的 Mod 并切过去（`group` 给了就是切到已有的那组）
#[tauri::command]
pub async fn mp_join_group(app: AppHandle, state: State<'_, AppState>, code: String, dir: String, name: String, group: Option<String>) -> CmdResult<u64> {
    let inv = mp::decode(&code).map_err(err)?;
    crate::instance::check_name(&name).map_err(err)?;
    let root = crate::commands::instance::root_of(&state, &dir)?;
    let (gd, isolated) = crate::instance::game_dir(&root, &name);
    if !isolated {
        return Err("这个实例没开版本隔离，不能分 Mod 组，照着邀请码新建一个吧".into());
    }
    crate::commands::modgroups::ensure_idle(&state, &dir, &name)?;
    let title = if group.is_some() { format!("给「{name}」换 Mod 组") } else { format!("给「{name}」建一组朋友的 Mod") };
    let task = state.tasks.create(title, format!("{} {}", inv.mc, inv.loader), String::new());
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let net = state.net.clone();
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            let mut note = String::new();
            let gid = match group {
                Some(g) => g,
                None => {
                    task.stage("看本机已有的 Mod");
                    let gd2 = gd.clone();
                    let have = tokio::task::spawn_blocking(move || modgroups::all_files_by_sha1(&gd2)).await?;
                    let gname = modgroups::unique_name(&gd, &format!("{}（邀请码）", inv.name));
                    let gid = modgroups::create(&gd, &gname, false, Some(format!("邀请码 · {}", inv.name)))?;
                    let gdir = modgroups::dir_of(&gd, &modgroups::load(&gd), &gid);
                    match fetch_mods(&net, &gdir, &inv, &task, &have).await {
                        Ok(n) => note = n,
                        Err(e) => {
                            modgroups::discard(&gd, &gid);
                            return Err(e);
                        }
                    }
                    gid
                }
            };
            task.stage("切换 Mod 组");
            if let Err(e) = crate::commands::modgroups::ensure_idle(&app.state::<AppState>(), &dir, &name) {
                anyhow::bail!("{e}");
            }
            modgroups::switch(&gd, &gid)?;
            Ok(Some(format!("「{name}」换好了朋友的 Mod{note}")))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("content-installed", json!({ "dir": dir, "name": name, "kind": "mod" }));
    });
    Ok(id)
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
            if !inv.mods.is_empty() || !inv.cf.is_empty() || !inv.miss.is_empty() {
                // 装进实例自己的 mods（建了这个文件夹，实例就是版本隔离的，不会混进公共 mods）
                let mods_dir = PathBuf::from(&target.path).join("versions").join(&name).join("mods");
                note = fetch_mods(&net, &mods_dir, &inv, &task, &HashMap::new()).await?;
            }
            Ok(Some(format!("建好了「{name}」{note}")))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}
