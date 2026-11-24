//! 账户：列出、添加离线账户（按 D6）、微软设备代码登录、删除、设为默认。
//! 微软登录：`msa_begin` 返回要在网页上输入的代码，后台轮询，结果用事件 `msa-result` 推回来：
//! { ok: true, account } 或 { ok: false, error }。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

use crate::auth::{msa, AccountView};
use crate::base::util::now_ms;
use crate::state::{err, AppState, CmdResult};

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> Vec<AccountView> {
    state.auth.views()
}

#[tauri::command]
pub async fn offline_allowed(state: State<'_, AppState>) -> CmdResult<Value> {
    let (ok, reason) = state.auth.offline_allowed().await;
    Ok(json!({ "ok": ok, "reason": reason }))
}

#[tauri::command]
pub async fn add_offline_account(state: State<'_, AppState>, name: String) -> CmdResult<AccountView> {
    state.auth.add_offline(&name).await.map_err(err)
}

#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, id: String) -> Vec<AccountView> {
    state.auth.remove(&id);
    state.auth.views()
}

#[tauri::command]
pub fn set_default_account(state: State<'_, AppState>, id: String) -> CmdResult<Vec<AccountView>> {
    state.auth.set_default(&id).map_err(err)?;
    Ok(state.auth.views())
}

#[tauri::command]
pub async fn msa_begin(app: AppHandle, state: State<'_, AppState>) -> CmdResult<msa::DeviceCode> {
    let http = msa::client().map_err(err)?;
    let dc = msa::device_code(&http).await.map_err(err)?;
    // 只留一个在等的登录
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(old) = state.msa_cancel.lock().unwrap().replace(cancel.clone()) {
        old.store(true, Ordering::Relaxed);
    }
    let auth = state.auth.clone();
    let (device_code, mut interval, deadline) = (dc.device_code.clone(), dc.interval.max(2), now_ms() + dc.expires_in * 1000);
    tauri::async_runtime::spawn(async move {
        let result: anyhow::Result<AccountView> = async {
            loop {
                tokio::time::sleep(Duration::from_secs(interval)).await;
                if cancel.load(Ordering::Relaxed) {
                    anyhow::bail!("取消了");
                }
                if now_ms() > deadline {
                    anyhow::bail!("代码过期了，重新点一次登录");
                }
                match msa::poll(&http, &device_code).await? {
                    msa::Poll::Pending => {}
                    msa::Poll::SlowDown => interval += 5,
                    msa::Poll::Done(tok) => return auth.finish_msa(&http, tok).await,
                }
            }
        }
        .await;
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let _ = match result {
            Ok(a) => app.emit("msa-result", json!({ "ok": true, "account": a })),
            Err(e) => app.emit("msa-result", json!({ "ok": false, "error": format!("{:#}", e) })),
        };
    });
    Ok(dc)
}

#[tauri::command]
pub fn msa_cancel(state: State<'_, AppState>) {
    if let Some(c) = state.msa_cancel.lock().unwrap().take() {
        c.store(true, Ordering::Relaxed);
    }
}
