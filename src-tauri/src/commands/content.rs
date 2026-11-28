//! 资源中心：搜索、详情、版本、装进实例（带前置）、检查更新。

use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

use crate::base::task;
use crate::commands::instance::root_of;
use crate::content::{curseforge, install, modrinth, zhname};
use crate::state::{err, AppState, CmdResult};

/// 实例的游戏目录、MC 版本、加载器（Modrinth 的写法：fabric / forge / neoforge / quilt）
fn inst(state: &AppState, dir: &str, name: &str) -> CmdResult<(PathBuf, Option<String>, Option<String>)> {
    let root = root_of(state, dir)?;
    crate::instance::check_name(name).map_err(err)?;
    let (mc, loader) = state
        .targets
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == dir)
        .and_then(|t| t.versions.iter().find(|v| v.name == name))
        .map(|v| (v.mc.clone(), Some(v.loader.to_lowercase()).filter(|l| !l.is_empty())))
        .unwrap_or((None, None));
    Ok((crate::instance::game_dir(&root, name).0, mc, loader))
}

async fn search_one(http: &reqwest::Client, q: &modrinth::Query) -> anyhow::Result<(Vec<modrinth::Hit>, u64)> {
    if q.source == "curseforge" {
        curseforge::search(q).await
    } else {
        modrinth::search(http, q).await
    }
}

/// 用中文搜：先认出可能是哪几个 Mod（自带的中文名 + MC百科），按英文名分头搜，只留名字对得上的；
/// 后面接上第一个名字搜出来的其他结果（附属之类），凑满一页
async fn search_zh(http: &reqwest::Client, q: &modrinth::Query) -> Vec<modrinth::Hit> {
    let names = zhname::candidates(http, &q.text).await;
    if names.is_empty() {
        return vec![];
    }
    let runs = names.iter().map(|n| {
        let mut q2 = q.clone();
        q2.text = n.en.clone();
        q2.sort = "relevance".into();
        async move { search_one(http, &q2).await.ok() }
    });
    let res = futures_util::future::join_all(runs).await;
    let mut out = vec![];
    let mut seen = std::collections::HashSet::new();
    for (n, r) in names.iter().zip(&res) {
        let Some((hits, _)) = r else { continue };
        if let Some(h) = hits.iter().find(|h| zhname::matches(n, &h.title, &h.slug)) {
            if seen.insert(h.id.clone()) {
                let mut h = h.clone();
                h.zh = Some(n.zh.clone()).filter(|z| !z.is_empty());
                out.push(h);
            }
        }
    }
    if out.is_empty() {
        return out;
    }
    if let Some(Some((hits, _))) = res.first() {
        for h in hits {
            if out.len() >= 20 {
                break;
            }
            if seen.insert(h.id.clone()) {
                let mut h = h.clone();
                // 附属里正好也是 MC百科给过名字的，也标上中文名
                h.zh = names.iter().find(|n| !n.zh.is_empty() && zhname::matches(n, &h.title, &h.slug)).map(|n| n.zh.clone());
                out.push(h);
            }
        }
    }
    out
}

#[tauri::command]
pub async fn content_search(q: modrinth::Query) -> CmdResult<Value> {
    let http = modrinth::client().map_err(err)?;
    if q.offset == 0 && matches!(q.kind.as_str(), "" | "mod") && zhname::has_cjk(&q.text) {
        let mut hits = search_zh(&http, &q).await;
        // 认出来了但没有适合这个实例的版本（比如机械动力 1.21.1 只有 NeoForge 版）：照样列出来，告诉玩家装不上，别只说「没搜到」
        let mut unfit = false;
        if hits.is_empty() && (q.mc.is_some() || q.loader.is_some()) {
            let mut q2 = q.clone();
            q2.mc = None;
            q2.loader = None;
            hits = search_zh(&http, &q2).await;
            unfit = !hits.is_empty();
        }
        if !hits.is_empty() {
            zhname::annotate(&mut hits);
            let total = hits.len();
            return Ok(json!({ "hits": hits, "total": total, "via": "mcmod", "unfit": unfit }));
        }
    }
    let (mut hits, total) = search_one(&http, &q).await.map_err(err)?;
    zhname::annotate(&mut hits);
    Ok(json!({ "hits": hits, "total": total }))
}

#[tauri::command]
pub async fn content_project(id: String, source: Option<String>) -> CmdResult<Value> {
    if source.as_deref() == Some("curseforge") {
        return curseforge::project(&id).await.map_err(err);
    }
    modrinth::project(&modrinth::client().map_err(err)?, &id).await.map_err(err)
}

/// CurseForge 代理部署了没有
#[tauri::command]
pub async fn cf_available() -> bool {
    curseforge::ready().await
}

#[tauri::command]
pub async fn content_versions(id: String, mc: Option<String>, loader: Option<String>, source: Option<String>) -> CmdResult<Vec<modrinth::Version>> {
    if source.as_deref() == Some("curseforge") {
        return curseforge::versions(&id, mc.as_deref(), loader.as_deref()).await.map_err(err);
    }
    modrinth::versions(&modrinth::client().map_err(err)?, &id, mc.as_deref(), loader.as_deref()).await.map_err(err)
}

/// 装之前先算：装哪个版本、要带哪些前置、换掉哪些旧文件
#[tauri::command]
pub async fn content_plan(state: State<'_, AppState>, dir: String, name: String, kind: String, project: String, version: Option<String>, source: Option<String>) -> CmdResult<Vec<install::Item>> {
    let (gd, mc, loader) = inst(&state, &dir, &name)?;
    let loader = if kind == "mod" { loader } else { None };
    if source.as_deref() == Some("curseforge") {
        // 本地已经有的 CurseForge 项目：按指纹认
        let folder = install::dir_of(&gd, &kind);
        let fps: Vec<u32> = tauri::async_runtime::spawn_blocking(move || {
            std::fs::read_dir(&folder).into_iter().flatten().flatten().filter_map(|e| std::fs::read(e.path()).ok()).map(|b| curseforge::fingerprint(&b)).collect()
        })
        .await
        .map_err(|e| e.to_string())?;
        let installed: Vec<String> = curseforge::by_fingerprints(&fps).await.map(|m| m.into_values().collect()).unwrap_or_default();
        return curseforge::plan(&project, version.as_deref(), mc.as_deref(), loader.as_deref(), &installed).await.map_err(err);
    }
    let http = modrinth::client().map_err(err)?;
    let folder = install::dir_of(&gd, &kind);
    install::plan(&http, &folder, &project, version.as_deref(), mc.as_deref(), loader.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn content_install(app: AppHandle, state: State<'_, AppState>, dir: String, name: String, kind: String, items: Vec<install::Item>) -> CmdResult<u64> {
    let (gd, _, _) = inst(&state, &dir, &name)?;
    if items.is_empty() {
        return Err("没有要装的".into());
    }
    let main = items.iter().find(|i| !i.dep).or(items.first()).map(|i| i.title.clone()).unwrap_or_default();
    let deps = items.iter().filter(|i| i.dep).count();
    let task = state.tasks.create(format!("装「{main}」{}", if deps > 0 { format!("和 {deps} 个前置") } else { String::new() }), name.clone(), String::new());
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let net = state.net.clone();
    let folder = install::dir_of(&gd, &kind);
    tauri::async_runtime::spawn(async move {
        task.stage("下载");
        let r = install::apply(&net, &folder, &items, &task.prog).await.map(|files| Some(format!("装好了：{}", files.join("、"))));
        task::finish(&app, &task, r);
        let _ = app.emit("content-installed", json!({ "dir": dir, "name": name, "kind": kind }));
    });
    Ok(id)
}

/// 检查更新：mods 文件夹里 Modrinth 认得的文件，在这个游戏版本和加载器下有没有更新的版本
#[tauri::command]
pub async fn mod_updates(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<Value> {
    let (gd, mc, loader) = inst(&state, &dir, &name)?;
    let mc = mc.ok_or("认不出这个实例的游戏版本，检查不了更新")?;
    let folder = gd.join("mods");
    let local = tauri::async_runtime::spawn_blocking(move || install::local_hashes(&folder)).await.map_err(|e| e.to_string())?;
    let hashes: Vec<String> = local.keys().cloned().collect();
    let http = modrinth::client().map_err(err)?;
    let now = modrinth::by_hashes(&http, &hashes).await.map_err(err)?;
    let newest = modrinth::updates(&http, &hashes, &mc, loader.as_deref()).await.map_err(err)?;
    let names = modrinth::projects(&http, &now.values().map(|v| v.project_id.clone()).collect::<Vec<_>>()).await.unwrap_or_default();
    let mut list = vec![];
    for (h, cur) in &now {
        let Some(new) = newest.get(h) else { continue };
        if new.id == cur.id || new.date <= cur.date {
            continue;
        }
        let Some(file) = new.primary().cloned() else { continue };
        let file_name = local.get(h).cloned().unwrap_or_default();
        list.push(json!({
            "file": file_name,
            "title": names.get(&cur.project_id).and_then(|p| p["title"].as_str()).unwrap_or(&cur.name),
            "from": cur.number, "to": new.number, "kind": new.kind,
            "item": install::Item { project: cur.project_id.clone(), title: names.get(&cur.project_id).and_then(|p| p["title"].as_str()).unwrap_or("").into(), version: new.number.clone(), file, dep: false, replaces: Some(file_name.clone()) },
        }));
    }
    Ok(json!({ "updates": list, "known": now.len(), "total": local.len() }))
}

// ---------- 低配优化包、显卡 ----------

/// 显卡、这个实例能加哪些优化 Mod、已经加了哪些
#[tauri::command]
pub async fn perf_status(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<Value> {
    let (gd, mc, loader) = inst(&state, &dir, &name)?;
    let root = root_of(&state, &dir)?;
    let cfg = crate::instance::load_config(&root, &name);
    let gpus = tauri::async_runtime::spawn_blocking(crate::base::gpu::list).await.unwrap_or_default();
    let entries = match (mc, loader) {
        (Some(mc), Some(l)) => crate::content::perf::check(&modrinth::client().map_err(err)?, &gd.join("mods"), &mc, &l).await.map_err(err)?,
        _ => vec![],
    };
    let added: Vec<&String> = cfg.perf_mods.iter().filter(|f| gd.join("mods").join(f).is_file()).collect();
    Ok(json!({ "gpus": gpus, "entries": entries, "added": added }))
}

/// 这台电脑的显卡（概览卡片用；只读一次）
#[tauri::command]
pub async fn gpu_list() -> Vec<crate::base::gpu::Gpu> {
    static CACHE: std::sync::OnceLock<Vec<crate::base::gpu::Gpu>> = std::sync::OnceLock::new();
    if let Some(v) = CACHE.get() {
        return v.clone();
    }
    let v = tauri::async_runtime::spawn_blocking(crate::base::gpu::list).await.unwrap_or_default();
    CACHE.get_or_init(|| v).clone()
}

/// 加上选中的优化 Mod（带前置），记下加了哪些文件
#[tauri::command]
pub async fn perf_apply(app: AppHandle, state: State<'_, AppState>, dir: String, name: String, slugs: Vec<String>) -> CmdResult<u64> {
    let (gd, mc, loader) = inst(&state, &dir, &name)?;
    let root = root_of(&state, &dir)?;
    let (mc, loader) = (mc.ok_or("认不出游戏版本")?, loader.ok_or("原版装不了 Mod")?);
    let task = state.tasks.create(format!("给「{name}」加低配优化包"), name.clone(), String::new());
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let net = state.net.clone();
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            let http = modrinth::client()?;
            let mods = gd.join("mods");
            let mut items: Vec<install::Item> = vec![];
            for slug in &slugs {
                task.stage(format!("查 {slug}"));
                match install::plan(&http, &mods, slug, None, Some(&mc), Some(&loader)).await {
                    Ok(list) => {
                        for it in list {
                            if !items.iter().any(|x| x.project == it.project) {
                                items.push(it);
                            }
                        }
                    }
                    Err(e) => task.log(format!("{slug}：{e:#}")),
                }
            }
            if items.is_empty() {
                anyhow::bail!("没有能加的");
            }
            task.stage("下载");
            let files = install::apply(&net, &mods, &items, &task.prog).await?;
            let mut cfg = crate::instance::load_config(&root, &name);
            for f in &files {
                if !cfg.perf_mods.contains(f) {
                    cfg.perf_mods.push(f.clone());
                }
            }
            crate::instance::save_config(&root, &name, &cfg)?;
            Ok(Some(format!("加了 {} 个：{}", files.len(), files.join("、"))))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("content-installed", json!({ "dir": dir, "name": name, "kind": "mod" }));
    });
    Ok(id)
}

/// 撤掉低配优化包：只把我们加的那几个放进回收站
#[tauri::command]
pub fn perf_remove(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<usize> {
    let (gd, _, _) = inst(&state, &dir, &name)?;
    let root = root_of(&state, &dir)?;
    let mut cfg = crate::instance::load_config(&root, &name);
    let mut n = 0;
    for f in &cfg.perf_mods {
        for cand in [f.clone(), format!("{f}.disabled")] {
            let p = gd.join("mods").join(&cand);
            if p.is_file() && crate::instance::to_recycle_bin(&p).is_ok() {
                n += 1;
            }
        }
    }
    cfg.perf_mods.clear();
    crate::instance::save_config(&root, &name, &cfg).map_err(err)?;
    Ok(n)
}

// ---------- 整合包 ----------

/// 选一个整合包文件
#[tauri::command]
pub async fn pack_pick(app: AppHandle) -> CmdResult<Option<String>> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog().file().set_title("选择整合包").add_filter("整合包", &["mrpack", "zip"]).pick_file(move |f| {
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

/// 看看整合包里是什么（装之前给用户确认）
#[tauri::command]
pub async fn pack_read(path: String) -> CmdResult<Value> {
    let p = crate::content::pack::read(std::path::Path::new(&path)).await.map_err(err)?;
    Ok(json!({ "format": p.format, "name": p.name, "version": p.version, "mc": p.mc, "loader": p.loader, "files": p.files.len(), "manual": p.manual.len() }))
}

fn pack_task(app: &AppHandle, state: &AppState, dir: &str, name: &str, title: String) -> CmdResult<(crate::game::launchers::Target, std::sync::Arc<crate::base::task::TaskEntry>, u64)> {
    crate::instance::check_name(name).map_err(err)?;
    let target = state.targets.lock().unwrap().iter().find(|t| t.id == dir && t.access == "direct").cloned().ok_or("找不到这个游戏目录")?;
    if std::path::Path::new(&target.path).join("versions").join(name).exists() {
        return Err(format!("已经有叫「{name}」的实例了，换个名字"));
    }
    let task = state.tasks.create(title, name.to_string(), format!("{} · {}", target.launcher, target.path));
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    Ok((target, task, id))
}

/// 从文件装整合包，成为一个新实例
#[tauri::command]
pub async fn pack_install(app: AppHandle, state: State<'_, AppState>, path: String, dir: String, name: String) -> CmdResult<u64> {
    let (target, task, id) = pack_task(&app, &state, &dir, &name, format!("装整合包「{name}」"))?;
    let env = crate::game::install::Env { app: Some(app.clone()), net: state.net.clone(), data_dir: state.data_dir.clone() };
    tauri::async_runtime::spawn(async move {
        let zip = std::path::PathBuf::from(&path);
        let r = async {
            task.stage("读整合包");
            let plan = crate::content::pack::read(&zip).await?;
            crate::content::pack::install(&env, target, &plan, &zip, &name, task.clone()).await
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}

/// 从 Modrinth 装整合包：下 .mrpack 再装
#[tauri::command]
pub async fn pack_install_modrinth(app: AppHandle, state: State<'_, AppState>, project: String, version: Option<String>, dir: String, name: String) -> CmdResult<u64> {
    let (target, task, id) = pack_task(&app, &state, &dir, &name, format!("装整合包「{name}」"))?;
    let env = crate::game::install::Env { app: Some(app.clone()), net: state.net.clone(), data_dir: state.data_dir.clone() };
    tauri::async_runtime::spawn(async move {
        let r = async {
            task.stage("找整合包的版本");
            let http = modrinth::client()?;
            let v = match version {
                Some(v) => modrinth::version(&http, &v).await?,
                None => modrinth::pick(&modrinth::versions(&http, &project, None, None).await?).cloned().ok_or_else(|| anyhow::anyhow!("这个整合包没有能下的版本"))?,
            };
            let f = v.primary().cloned().ok_or_else(|| anyhow::anyhow!("这个版本没有文件"))?;
            let dl = env.data_dir.join("packs");
            std::fs::create_dir_all(&dl)?;
            let zip = dl.join(install::safe_name(&f.filename)?);
            task.stage(format!("下载 {}", f.filename));
            crate::game::install::download_all(&env.net, &[crate::base::net::FileJob { url: f.url.clone(), path: zip.clone(), sha1: Some(f.sha1.clone()).filter(|s| !s.is_empty()), size: Some(f.size) }], &task.prog).await?;
            let plan = crate::content::pack::read(&zip).await?;
            let r = crate::content::pack::install(&env, target.clone(), &plan, &zip, &name, task.clone()).await;
            // 记下 Modrinth 项目，以后能检查整合包更新
            let root = std::path::PathBuf::from(&target.path);
            let mut cfg = crate::instance::load_config(&root, &name);
            if let Some(p) = cfg.pack.as_mut() {
                p["project"] = json!(project);
                p["versionId"] = json!(v.id);
            }
            let _ = crate::instance::save_config(&root, &name, &cfg);
            let _ = std::fs::remove_file(&zip);
            r
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}

/// 整合包有没有新版本（只认从 Modrinth 装的；别的可以用「从文件更新」）
#[tauri::command]
pub async fn pack_check(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<Value> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    let cfg = crate::instance::load_config(&root, &name);
    let Some(pack) = cfg.pack else { return Ok(json!({ "pack": false })) };
    let Some(project) = pack["project"].as_str().map(String::from) else {
        return Ok(json!({ "pack": true, "name": pack["name"], "current": pack["version"], "source": "file" }));
    };
    let (_, mc, _) = inst(&state, &dir, &name)?;
    let http = modrinth::client().map_err(err)?;
    let list = modrinth::versions(&http, &project, None, None).await.map_err(err)?;
    // 默认只推荐同一个游戏版本里最新的；换了游戏版本的另外列出来（存档升上去就回不来了）
    let same: Vec<_> = list.iter().filter(|v| mc.as_ref().is_some_and(|m| v.game_versions.contains(m))).cloned().collect();
    let latest = modrinth::pick(&same).cloned();
    let any = modrinth::pick(&list).cloned().ok_or("这个整合包在 Modrinth 上没有能下的版本")?;
    let cur_id = pack["versionId"].as_str();
    let other = (latest.as_ref().map(|l| l.id != any.id).unwrap_or(true) && Some(any.id.as_str()) != cur_id && !any.game_versions.iter().any(|g| Some(g) == mc.as_ref()))
        .then(|| json!({ "latest": any.number, "latestId": any.id, "mc": any.game_versions }));
    Ok(json!({
        "pack": true, "name": pack["name"], "current": pack["version"], "source": "modrinth", "mcNow": mc,
        "latest": latest.as_ref().map(|l| l.number.clone()), "latestId": latest.as_ref().map(|l| l.id.clone()),
        "newer": latest.as_ref().is_some_and(|l| Some(l.id.as_str()) != cur_id), "other": other,
    }))
}

/// 更新整合包：Modrinth 上的版本（version）或者一个新的整合包文件（file）。
/// 玩家改过的配置、自己加的 Mod 不动；换下来的旧文件放进 LeafStarter/pack-backup/
#[tauri::command]
pub async fn pack_update(app: AppHandle, state: State<'_, AppState>, dir: String, name: String, version: Option<String>, file: Option<String>) -> CmdResult<u64> {
    crate::commands::modgroups::ensure_idle(&state, &dir, &name)?;
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    let target = state.targets.lock().unwrap().iter().find(|t| t.id == dir && t.access == "direct").cloned().ok_or("找不到这个游戏目录")?;
    let cur = target.versions.iter().find(|v| v.name == name).cloned().ok_or("找不到这个实例")?;
    let cfg = crate::instance::load_config(&root, &name);
    let pack = cfg.pack.clone().ok_or("这个实例不是从整合包装的")?;
    let task = state.tasks.create(format!("更新整合包「{name}」"), name.clone(), String::new());
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let env = crate::game::install::Env { app: Some(app.clone()), net: state.net.clone(), data_dir: state.data_dir.clone() };
    tauri::async_runtime::spawn(async move {
        let r = async {
            let gd = crate::instance::game_dir(&root, &name).0;
            let dl = env.data_dir.join("packs");
            std::fs::create_dir_all(&dl)?;
            let http = modrinth::client()?;
            // 下一个 Modrinth 版本的 .mrpack（新版本；没记清单的老实例还要旧版本来算清单）
            let fetch = |vid: String| {
                let (http, env, dl, task) = (http.clone(), &env, dl.clone(), task.clone());
                async move {
                    let v = modrinth::version(&http, &vid).await?;
                    let f = v.primary().cloned().ok_or_else(|| anyhow::anyhow!("这个版本没有文件"))?;
                    let zip = dl.join(install::safe_name(&f.filename)?);
                    task.stage(format!("下载 {}", f.filename));
                    crate::game::install::download_all(&env.net, &[crate::base::net::FileJob { url: f.url.clone(), path: zip.clone(), sha1: Some(f.sha1.clone()).filter(|s| !s.is_empty()), size: Some(f.size) }], &task.prog).await?;
                    anyhow::Ok((zip, v.id))
                }
            };
            let (zip, new_id, temp) = match (&file, &version) {
                (Some(f), _) => (std::path::PathBuf::from(f), None, false),
                (None, Some(v)) => {
                    let (z, id) = fetch(v.clone()).await?;
                    (z, Some(id), true)
                }
                (None, None) => {
                    let project = pack["project"].as_str().ok_or_else(|| anyhow::anyhow!("这个整合包不是从 Modrinth 装的，选一个新的整合包文件来更新"))?;
                    let list = modrinth::versions(&http, project, None, None).await?;
                    let v = modrinth::pick(&list).cloned().ok_or_else(|| anyhow::anyhow!("没有能下的版本"))?;
                    let (z, id) = fetch(v.id).await?;
                    (z, Some(id), true)
                }
            };
            let plan = crate::content::pack::read(&zip).await?;
            // 旧清单：装的时候记的；没有（老版本 LeafStarter 装的）就把旧版本的整合包下下来算
            let old = match crate::content::pack::load_manifest(&gd) {
                Some(m) => m,
                None => match pack["versionId"].as_str() {
                    Some(vid) => {
                        task.stage("下旧版本，分出哪些是你自己改的");
                        let (oz, _) = fetch(vid.to_string()).await?;
                        let m = crate::content::pack::manifest_of(&crate::content::pack::read(&oz).await?, &oz)?;
                        let _ = std::fs::remove_file(&oz);
                        m
                    }
                    None => crate::content::pack::Manifest { version: pack["version"].as_str().unwrap_or("").into(), ..Default::default() },
                },
            };
            // 游戏或加载器版本变了：先把版本换过去（Mod、配置、存档在版本文件夹里，不受影响）
            let want_loader = plan.loader.as_ref().map(|(k, v)| (k.to_lowercase(), v.clone()));
            let have_loader = Some(cur.loader.to_lowercase()).filter(|l| !l.is_empty()).map(|l| (l, cur.loader_version.clone().unwrap_or_default()));
            let same_loader = match (&want_loader, &have_loader) {
                (Some((wk, wv)), Some((hk, hv))) => wk == hk && (hv.contains(wv.as_str()) || wv.contains(hv.as_str())),
                (None, None) => true,
                _ => false,
            };
            if cur.mc.as_deref() != Some(plan.mc.as_str()) || !same_loader {
                task.stage(format!("换成 {} {}", plan.mc, plan.loader.as_ref().map(|l| format!("{} {}", l.0, l.1)).unwrap_or_default()));
                let loader = match &plan.loader {
                    Some((kind, ver)) => {
                        let k = crate::game::loaders::LoaderKind::parse(kind).ok_or_else(|| anyhow::anyhow!("不支持的加载器 {kind}"))?;
                        let list = crate::game::loaders::list(&env.net, k, &plan.mc).await?;
                        let v = list.into_iter().find(|x| &x.version == ver || x.id.contains(ver.as_str())).ok_or_else(|| anyhow::anyhow!("找不到 {kind} {ver}"))?;
                        Some(crate::game::install::LoaderReq { kind: k.label().into(), id: v.id, version: v.version })
                    }
                    None => None,
                };
                crate::game::install::run(&env, target.clone(), crate::game::install::InstallRequest { target_id: target.id.clone(), mc_version: plan.mc.clone(), name: name.clone(), loader, overwrite: true }, task.clone()).await?;
            }
            let s = crate::content::pack::update(&env, &gd, &old, &plan, &zip, &task).await?;
            if temp {
                let _ = std::fs::remove_file(&zip);
            }
            let mut cfg = crate::instance::load_config(&root, &name);
            if let Some(p) = cfg.pack.as_mut() {
                p["version"] = json!(plan.version);
                if let Some(id) = &new_id {
                    p["versionId"] = json!(id);
                }
            }
            crate::instance::save_config(&root, &name, &cfg)?;
            let mut msg = format!("{} → {}：新加 {}、换掉 {}、删掉 {} 个文件", if s.from.is_empty() { "旧版本" } else { &s.from }, s.to, s.added, s.replaced, s.removed);
            if !s.kept.is_empty() {
                msg.push_str(&format!("；你改过的 {} 个配置没动，新版本的另存成 .new：{}", s.kept.len(), s.kept.iter().take(5).cloned().collect::<Vec<_>>().join("、")));
            }
            if s.replaced + s.removed > 0 {
                msg.push_str("；换下来的旧文件在 LeafStarter/pack-backup");
            }
            anyhow::Ok(Some(msg))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("targets-dirty", ());
        let _ = app.emit("content-installed", json!({ "dir": dir, "name": name, "kind": "mod" }));
    });
    Ok(id)
}

/// 导出成 .mrpack，放在桌面上
#[tauri::command]
pub async fn pack_export(state: State<'_, AppState>, dir: String, name: String, saves: bool) -> CmdResult<Value> {
    let (gd, mc, _) = inst(&state, &dir, &name)?;
    let mc = mc.ok_or("认不出游戏版本，导出不了")?;
    let (loader, lver) = state
        .targets
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == dir)
        .and_then(|t| t.versions.iter().find(|v| v.name == name))
        .map(|v| (v.loader.clone(), v.loader_version.clone().unwrap_or_default()))
        .unwrap_or_default();
    let out = dirs::desktop_dir().unwrap_or_else(|| gd.clone()).join(format!("{name}.mrpack"));
    let s = crate::content::export::export(&gd, &out, &name, "1.0.0", &mc, (!loader.is_empty()).then_some((loader.as_str(), lver.as_str())), saves).await.map_err(err)?;
    Ok(json!({ "path": out.to_string_lossy(), "linked": s.linked, "bundled": s.bundled }))
}
