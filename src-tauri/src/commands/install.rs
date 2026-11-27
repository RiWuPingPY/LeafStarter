//! 安装一个版本（可带加载器）到选中的游戏目录

use tauri::{AppHandle, Emitter, State};

use crate::base::task;
use crate::game::install::{self, Env, InstallRequest};
use crate::state::{AppState, CmdResult};

#[tauri::command]
pub async fn start_install(app: AppHandle, state: State<'_, AppState>, req: InstallRequest) -> CmdResult<u64> {
    let target = state
        .targets
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == req.target_id)
        .cloned()
        .ok_or("找不到目标目录，请重新扫描启动器")?;
    let ver = match &req.loader {
        Some(l) => format!("{} · {} {}", req.mc_version, l.kind, l.version),
        None => req.mc_version.clone(),
    };
    let task = state.tasks.create(format!("安装「{}」", req.name), ver, format!("{} · {}", target.launcher, target.path));
    let id = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let env = Env { app: Some(app.clone()), net: state.net.clone(), data_dir: state.data_dir.clone() };
    tauri::async_runtime::spawn(async move {
        // run 返回的是装好的路径，界面用不着；任务的结果说明留空
        let result = install::run(&env, target, req, task.clone()).await.map(|_| None);
        task::finish(&app, &task, result);
        let _ = app.emit("targets-dirty", ());
    });
    Ok(id)
}
