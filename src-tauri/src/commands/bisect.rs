//! 二分查冲突：后台一次次启动游戏，每次只开一部分 Mod，看崩不崩。进度用 `bisect-state` 事件推给界面：
//! { key, phase: running | done | failed | cancelled, step, total, testing, last, result, files, msg }
//! 结束（找到、取消、出错）后把每个 Mod 的开关还原成开始时的样子。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::instance::{loader_of, root_of};
use crate::game::modmeta::{self, ModMeta};
use crate::launch::{self, bisect, process};
use crate::state::{err, AppState, CmdResult};

/// 正在查的实例 key → 取消开关
pub type Running = Mutex<HashMap<String, Arc<AtomicBool>>>;

#[tauri::command]
pub fn bisect_estimate(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<Value> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    let mods: Vec<ModMeta> = modmeta::read_dir(&crate::instance::game_dir(&root, &name).0.join("mods"), &loader_of(&state, &dir, &name)).into_iter().filter(|m| m.enabled).collect();
    Ok(json!({ "mods": mods.len(), "launches": bisect::estimate(mods.len()) }))
}

#[tauri::command]
pub fn bisect_cancel(state: State<'_, AppState>, key: String) {
    if let Some(c) = state.bisects.lock().unwrap().get(&key) {
        c.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub async fn bisect_start(app: AppHandle, state: State<'_, AppState>, dir: String, name: String, key: String) -> CmdResult<()> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    if state.runs.lock().unwrap().values().any(|r| r.key == key) {
        return Err("游戏还开着，先关掉".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut b = state.bisects.lock().unwrap();
        if b.contains_key(&key) {
            return Err("已经在查了".into());
        }
        b.insert(key.clone(), cancel.clone());
    }
    let mods_dir = crate::instance::game_dir(&root, &name).0.join("mods");
    let all: Vec<ModMeta> = modmeta::read_dir(&mods_dir, &loader_of(&state, &dir, &name)).into_iter().filter(|m| m.enabled).collect();
    if all.len() < 2 {
        state.bisects.lock().unwrap().remove(&key);
        return Err("这个实例开着的 Mod 不到两个，不用二分".into());
    }
    let total = bisect::estimate(all.len());
    let emit = {
        let (app, key) = (app.clone(), key.clone());
        move |v: Value| {
            let mut v = v;
            v["key"] = json!(key);
            let _ = app.emit("bisect-state", v);
        }
    };

    tauri::async_runtime::spawn(async move {
        // 现在每个 Mod 的文件名（开关会改名：xxx.jar ↔ xxx.jar.disabled）
        let names = Arc::new(Mutex::new(all.iter().map(|m| m.file.clone()).collect::<Vec<_>>()));
        let step = Arc::new(Mutex::new(0usize));
        let last = Arc::new(Mutex::new(Value::Null));
        let st = app.state::<AppState>();
        let (net, data_dir, auth) = (st.net.clone(), st.data_dir.clone(), st.auth.clone());
        let opts = crate::instance::load_config(&root, &name);

        let outcome = bisect::search(all.len(), |pick| {
            let (names, step, last, emit, cancel, all) = (names.clone(), step.clone(), last.clone(), emit.clone(), cancel.clone(), all.clone());
            let (root, name, net, data_dir, auth, opts) = (root.clone(), name.clone(), net.clone(), data_dir.clone(), auth.clone(), opts.clone());
            async move {
                if cancel.load(Ordering::Relaxed) {
                    anyhow::bail!("取消了");
                }
                let on = modmeta::closure(&all, &pick);
                set_enabled(&root, &name, &names, &on)?;
                let n = {
                    let mut s = step.lock().unwrap();
                    *s += 1;
                    *s
                };
                emit(json!({ "phase": "running", "step": n, "total": total, "testing": on.len(), "of": all.len(), "last": *last.lock().unwrap() }));
                let a = auth.for_launch(None).await?;
                let report: launch::Report = Arc::new(|_: &str, _: u32, _: String| {});
                let p = launch::prepare(&net, &data_dir, &root, &name, &a, &opts, report).await?;
                let r = process::smoke(&p.java, &p.args, p.java_major, &p.game_dir, &p.natives_dir, Duration::from_secs(420), Some(cancel.clone()), None).await?;
                if cancel.load(Ordering::Relaxed) {
                    anyhow::bail!("取消了");
                }
                let crashed = !r.ok;
                *last.lock().unwrap() = json!({ "count": on.len(), "crashed": crashed, "timedOut": r.timed_out, "secs": r.secs.round() });
                Ok(crashed)
            }
        })
        .await;

        // 还原：开始时开着的全部重新打开
        let every: Vec<usize> = (0..all.len()).collect();
        let restored = set_enabled(&root, &name, &names, &every);
        let file = |k: usize| all[k].name.clone().unwrap_or_else(|| all[k].file.clone());
        let msg = match &outcome {
            Ok(bisect::Outcome::NoCrash) => "全部 Mod 开着这次没崩，可能是偶发的；再崩的时候看日志".to_string(),
            Ok(bisect::Outcome::NotMods) => "一个 Mod 都不开也崩，不是 Mod 冲突。看日志最后几行，或者检查 Java 和内存".to_string(),
            Ok(bisect::Outcome::Found { culprit, partner: None }) => format!("找到了：「{}」自己就会让游戏启动时崩溃", file(*culprit)),
            Ok(bisect::Outcome::Found { culprit, partner: Some(p) }) => format!("找到了：「{}」和「{}」放在一起会崩，单开哪个都没事", file(*culprit), file(*p)),
            Err(e) => format!("{e:#}"),
        };
        let cancelled = cancel.load(Ordering::Relaxed);
        let files: Vec<Value> = match &outcome {
            Ok(bisect::Outcome::Found { culprit, partner }) => std::iter::once(*culprit).chain(*partner).map(|k| json!({ "file": all[k].file, "name": file(k), "id": all[k].id })).collect(),
            _ => vec![],
        };
        let phase = if cancelled { "cancelled" } else if outcome.is_ok() { "done" } else { "failed" };
        let restore_note = restored.err().map(|e| format!("；有 Mod 没还原成原来的开关：{e:#}"));
        emit(json!({
            "phase": phase,
            "step": *step.lock().unwrap(),
            "total": total,
            "result": outcome.as_ref().ok(),
            "files": files,
            "msg": if cancelled { "取消了，Mod 开关已经还原".to_string() } else { msg + restore_note.as_deref().unwrap_or("") },
        }));
        app.state::<AppState>().bisects.lock().unwrap().remove(&key);
    });
    Ok(())
}

/// 只开 `on` 里的（按开始时的序号），其余关掉；改名后记下新文件名
fn set_enabled(root: &Path, name: &str, names: &Mutex<Vec<String>>, on: &[usize]) -> anyhow::Result<()> {
    let mut names = names.lock().unwrap();
    let mut first_err = None;
    for k in 0..names.len() {
        let want = on.contains(&k);
        let is_on = !names[k].to_ascii_lowercase().ends_with(".disabled");
        if want != is_on {
            match crate::instance::set_mod_enabled(root, name, &names[k], want) {
                Ok(n) => names[k] = n,
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
    }
    first_err.map_or(Ok(()), Err)
}
