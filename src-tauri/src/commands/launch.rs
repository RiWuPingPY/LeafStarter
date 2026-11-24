//! 启动和结束游戏。进度和日志用事件推给界面：
//! - `launch-state`：{ run, key, phase, pct, detail, java, exitCode, crashReport }，
//!   phase 依次是 account → files → java → starting → running，结束时是 exited / crashed / failed
//! - `game-log`：{ run, key, lines }，每 0.2 秒一批

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

use crate::base::util::now_ms;
use crate::commands::instance::root_of;
use crate::launch::{self, process};
use crate::state::{err, AppState, CmdResult, RunCtl};

const LOG_KEEP: usize = 5000;

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct StateEvent {
    run: u64,
    key: String,
    phase: String,
    pct: u32,
    detail: String,
    java: u32,
    exit_code: Option<i32>,
    crash_report: Option<String>,
    diag: Option<launch::crash::Diagnosis>,
}

#[derive(Clone, Serialize)]
struct LogEvent {
    run: u64,
    key: String,
    lines: Vec<String>,
}

#[tauri::command]
pub async fn launch_instance(app: AppHandle, state: State<'_, AppState>, dir: String, name: String, key: String, account: Option<String>, opts: Option<launch::Options>) -> CmdResult<u64> {
    let root = root_of(&state, &dir)?;
    crate::instance::check_name(&name).map_err(err)?;
    if state.runs.lock().unwrap().values().any(|r| r.key == key) {
        return Err("这个实例已经在运行了".into());
    }
    if state.bisects.lock().unwrap().contains_key(&key) {
        return Err("正在查是哪个 Mod，查完再启动".into());
    }
    crate::direct::remember(&state, &root, &name);
    let run = state.next_run.fetch_add(1, Ordering::SeqCst) + 1;
    let (kill_tx, kill_rx) = oneshot::channel();
    state.runs.lock().unwrap().insert(run, RunCtl { key: key.clone(), kill: Some(kill_tx) });
    let log = Arc::new(Mutex::new(VecDeque::<String>::new()));
    state.logs.lock().unwrap().insert(key.clone(), log.clone());

    let (net, data_dir, auth) = (state.net.clone(), state.data_dir.clone(), state.auth.clone());
    let opts = opts.unwrap_or_else(|| crate::instance::load_config(&root, &name));
    let (root2, name2, opts2) = (root.clone(), name.clone(), opts.clone());
    let java_major = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let send = {
        let (app, key, jm) = (app.clone(), key.clone(), java_major.clone());
        Arc::new(move |e: StateEvent| {
            let ev = StateEvent { run, key: key.clone(), java: jm.load(Ordering::Relaxed), ..e };
            if let Ok(v) = serde_json::to_value(&ev) {
                app.state::<AppState>().run_states.lock().unwrap().insert(key.clone(), v);
            }
            let _ = app.emit("launch-state", ev.clone());
            crate::direct::on_state(&app, &key, &ev.phase);
        })
    };

    tauri::async_runtime::spawn(async move {
        let outcome: anyhow::Result<(process::Exit, std::path::PathBuf, u64)> = async {
            send(StateEvent { phase: "account".into(), detail: "检查账户".into(), ..Default::default() });
            let a = auth.for_launch(account.as_deref()).await?;
            let report: launch::Report = {
                let (send, jm) = (send.clone(), java_major.clone());
                Arc::new(move |phase: &str, pct: u32, detail: String| {
                    if phase == "java" && pct == 100 {
                        if let Some(n) = detail.strip_prefix("Java ").and_then(|n| n.parse().ok()) {
                            jm.store(n, Ordering::Relaxed);
                        }
                    }
                    send(StateEvent { phase: phase.into(), pct, detail, ..Default::default() });
                })
            };
            let p = launch::prepare(&net, &data_dir, &root, &name, &a, &opts, report).await?;
            java_major.store(p.java_major, Ordering::Relaxed);
            send(StateEvent { phase: "starting".into(), detail: "等游戏窗口".into(), ..Default::default() });

            // 日志：存一份给「日志」标签，再按批推给界面；看到建窗口的日志就算运行起来了
            let seen = Arc::new(AtomicBool::new(false));
            let pending = Arc::new(Mutex::new(Vec::<String>::new()));
            let on_line: Arc<dyn Fn(String) + Send + Sync> = {
                let (log, pending, seen, send, token) = (log.clone(), pending.clone(), seen.clone(), send.clone(), a.token.clone());
                Arc::new(move |line: String| {
                    let line = launch::redact(&line, &token);
                    if !seen.load(Ordering::Relaxed) && process::is_window_line(&line) {
                        seen.store(true, Ordering::Relaxed);
                        send(StateEvent { phase: "running".into(), ..Default::default() });
                    }
                    let mut l = log.lock().unwrap();
                    l.push_back(line.clone());
                    while l.len() > LOG_KEEP {
                        l.pop_front();
                    }
                    pending.lock().unwrap().push(line);
                })
            };
            let started = now_ms();
            let ticker = {
                let (app, key, pending, seen, send) = (app.clone(), key.clone(), pending.clone(), seen.clone(), send.clone());
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        let lines = std::mem::take(&mut *pending.lock().unwrap());
                        if !lines.is_empty() {
                            let _ = app.emit("game-log", LogEvent { run, key: key.clone(), lines });
                        }
                        // 45 秒还没认出窗口，也当作运行起来了（有的整合包把日志改得认不出）
                        if !seen.load(Ordering::Relaxed) && now_ms() > started + 45_000 {
                            seen.store(true, Ordering::Relaxed);
                            send(StateEvent { phase: "running".into(), ..Default::default() });
                        }
                    }
                })
            };
            let exit = process::run_tuned(&p.java, &p.args, p.java_major, &p.game_dir, &p.natives_dir, kill_rx, on_line, p.tune).await;
            ticker.abort();
            let lines = std::mem::take(&mut *pending.lock().unwrap());
            if !lines.is_empty() {
                let _ = app.emit("game-log", LogEvent { run, key: key.clone(), lines });
            }
            Ok((exit?, p.game_dir, started))
        }
        .await;

        if let Ok((_, game_dir, started)) = &outcome {
            crate::commands::sync::after_run(app.state::<AppState>().store.clone(), game_dir.clone(), *started);
        }
        match outcome {
            Ok((exit, game_dir, started)) => {
                if exit.killed || exit.code == Some(0) {
                    send(StateEvent { phase: "exited".into(), exit_code: exit.code, detail: if exit.killed { "已结束".into() } else { "正常退出".into() }, ..Default::default() });
                } else {
                    let report = process::find_crash_report(&game_dir, started).map(|p| p.to_string_lossy().into_owned());
                    // 规则分析：崩溃报告 + 日志最后几千行
                    let text = report.as_deref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
                    let lines: Vec<String> = log.lock().unwrap().iter().cloned().collect();
                    let mods: Vec<String> = crate::instance::files(&root2, &name2).map(|f| f.mods.into_iter().filter(|m| m.enabled).map(|m| m.file).collect()).unwrap_or_default();
                    let total = launch::jvm::total_memory_mb();
                    let ctx = launch::crash::Ctx {
                        memory_mb: opts2.memory_mb.unwrap_or_else(|| launch::jvm::auto_memory_mb(total)),
                        total_mb: total,
                        java_major: java_major.load(Ordering::Relaxed),
                        java_custom: !opts2.java_path.trim().is_empty(),
                        mods: &mods,
                    };
                    let diag = launch::crash::analyze(&lines, &text, &ctx);
                    send(StateEvent { phase: "crashed".into(), exit_code: exit.code, crash_report: report, diag, detail: format!("退出码 {}", exit.code.map(|c| c.to_string()).unwrap_or_else(|| "未知".into())), ..Default::default() });
                }
            }
            Err(e) => send(StateEvent { phase: "failed".into(), detail: format!("{:#}", e), ..Default::default() }),
        }
        app.state::<AppState>().runs.lock().unwrap().remove(&run);
    });
    Ok(run)
}

#[tauri::command]
pub fn stop_instance(state: State<'_, AppState>, run: u64) {
    if let Some(r) = state.runs.lock().unwrap().get_mut(&run) {
        if let Some(tx) = r.kill.take() {
            let _ = tx.send(());
        }
    }
}

/// 某个实例最近一次运行的日志（切走再切回来时补上）
#[tauri::command]
pub fn game_log(state: State<'_, AppState>, key: String) -> Vec<String> {
    state.logs.lock().unwrap().get(&key).map(|l| l.lock().unwrap().iter().cloned().collect()).unwrap_or_default()
}

/// 界面刚加载（或者刷新了）时补上各实例最近的启动状态；已经结束的取走就删
#[tauri::command]
pub fn run_states(state: State<'_, AppState>) -> Vec<serde_json::Value> {
    let mut m = state.run_states.lock().unwrap();
    let out: Vec<_> = m.values().cloned().collect();
    m.retain(|_, v| !matches!(v["phase"].as_str(), Some("exited" | "crashed" | "failed")));
    out
}

/// 读崩溃报告的开头（界面上显示，完整的用「打开」看）
#[tauri::command]
pub fn read_crash_report(path: String) -> CmdResult<String> {
    let p = std::path::PathBuf::from(&path);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let in_reports = p.parent().and_then(|d| d.file_name()).is_some_and(|d| d == "crash-reports");
    if !(in_reports && name.ends_with(".txt")) && !(name.starts_with("hs_err_pid") && name.ends_with(".log")) {
        return Err("只能读崩溃报告".into());
    }
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    Ok(text.chars().take(20_000).collect())
}
