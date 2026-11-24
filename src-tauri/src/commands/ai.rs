//! AI 源、AI 助手会话、MCP 开关和一键接入

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

use crate::agent::{self, llm::Source};
use crate::state::{err, AppState, CmdResult};

fn view(st: &AppState) -> Value {
    let s = st.settings.lock().unwrap().ai.clone();
    let port = st.mcp_server.lock().unwrap().as_ref().map(|m| m.port);
    let sources: Vec<Value> = s
        .sources
        .iter()
        .map(|x| {
            let mut v = serde_json::to_value(x).unwrap_or_default();
            v["hasKey"] = json!(!x.key.is_empty());
            if let Some(o) = v.as_object_mut() {
                o.remove("key");
            }
            v
        })
        .collect();
    let levels = if s.mcp_levels.is_empty() { vec!["read".to_string(), "rev".to_string()] } else { s.mcp_levels.clone() };
    json!({
        "sources": sources,
        "uses": s.uses, "cap": s.cap, "mcp": s.mcp, "mcpLevels": levels, "mcpPort": port,
        "exe": std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
    })
}

#[tauri::command]
pub fn ai_get(state: State<'_, AppState>) -> Value {
    view(&state)
}

/// 新增或修改一个 AI 源；`key` 不传就保留原来的，传空字符串就清掉
#[tauri::command]
pub fn ai_save_source(state: State<'_, AppState>, source: Source, key: Option<String>) -> CmdResult<Value> {
    let mut src = source;
    if src.id.is_empty() {
        src.id = format!("ai{}", crate::base::util::now_ms());
    }
    if !["anthropic", "openai", "ollama"].contains(&src.kind.as_str()) {
        return Err("不认识的接口类型".into());
    }
    {
        let mut s = state.settings.lock().unwrap();
        let old = s.ai.sources.iter().find(|x| x.id == src.id).map(|x| x.key.clone()).unwrap_or_default();
        src.key = match key {
            Some(k) if !k.trim().is_empty() => crate::auth::vault::seal(k.trim().as_bytes()).map_err(err)?,
            Some(_) => String::new(),
            None => old,
        };
        let id = src.id.clone();
        s.ai.sources.retain(|x| x.id != id);
        s.ai.sources.push(src);
        s.ai.uses.entry("chat".into()).or_insert(id);
    }
    state.save();
    Ok(view(&state))
}

#[tauri::command]
pub fn ai_delete_source(state: State<'_, AppState>, id: String) -> Value {
    {
        let mut s = state.settings.lock().unwrap();
        s.ai.sources.retain(|x| x.id != id);
        s.ai.uses.retain(|_, v| *v != id);
    }
    state.save();
    view(&state)
}

#[tauri::command]
pub async fn ai_test(state: State<'_, AppState>, id: String) -> CmdResult<Value> {
    let src = state.settings.lock().unwrap().ai.sources.iter().find(|x| x.id == id).cloned().ok_or("找不到这个源")?;
    agent::llm::test(&src).await.map_err(err)
}

/// 改用途分配、上限、MCP 开关和开放的级别
#[tauri::command]
pub async fn ai_set(app: AppHandle, patch: Value) -> CmdResult<Value> {
    let st = app.state::<AppState>();
    let mcp_on = {
        let mut s = st.settings.lock().unwrap();
        if let Some(u) = patch.get("uses").and_then(|u| u.as_object()) {
            for (k, v) in u {
                if let Some(v) = v.as_str() {
                    s.ai.uses.insert(k.clone(), v.to_string());
                }
            }
        }
        if let Some(c) = patch.get("cap").and_then(|c| c.as_u64()) {
            s.ai.cap = c;
        }
        if let Some(l) = patch.get("mcpLevels").and_then(|l| l.as_array()) {
            s.ai.mcp_levels = l.iter().filter_map(|x| x.as_str()).filter(|x| ["read", "rev"].contains(x)).map(String::from).collect();
        }
        if let Some(m) = patch.get("mcp").and_then(|m| m.as_bool()) {
            s.ai.mcp = m;
        }
        s.ai.mcp
    };
    st.save();
    set_mcp(&app, mcp_on).await.map_err(err)?;
    Ok(view(&st))
}

pub async fn set_mcp(app: &AppHandle, on: bool) -> anyhow::Result<()> {
    let st = app.state::<AppState>();
    let running = st.mcp_server.lock().unwrap().is_some();
    if on && !running {
        let srv = agent::mcp::start(app.clone()).await?;
        *st.mcp_server.lock().unwrap() = Some(srv);
    } else if !on {
        st.mcp_server.lock().unwrap().take();
    }
    Ok(())
}

#[tauri::command]
pub fn agent_send(app: AppHandle, session: String, text: String) -> CmdResult<()> {
    if text.trim().is_empty() {
        return Err("说点什么".into());
    }
    tauri::async_runtime::spawn(async move {
        let _ = agent::send(app, session, text).await;
    });
    Ok(())
}

/// 回答确认：yes / all / no
#[tauri::command]
pub fn agent_answer(state: State<'_, AppState>, id: String, answer: String) {
    if let Some(tx) = state.agents.waiting.lock().unwrap().remove(&id) {
        let _ = tx.send(answer);
    }
}

#[tauri::command]
pub async fn agent_undo(app: AppHandle, session: String, step: String) -> CmdResult<String> {
    agent::undo(&app, &session, &step).await.map_err(err)
}

#[tauri::command]
pub fn agent_reset(state: State<'_, AppState>, session: String) {
    state.agents.sessions.lock().unwrap().remove(&session);
}

/// 常见 MCP 客户端的配置文件在哪
fn client_config(client: &str) -> Option<std::path::PathBuf> {
    match client {
        "claude-desktop" => dirs::config_dir().map(|d| d.join("Claude").join("claude_desktop_config.json")),
        "cursor" => dirs::home_dir().map(|d| d.join(".cursor").join("mcp.json")),
        _ => None,
    }
}

/// 一键接入：先看要改成什么样（`write = false`），确认后再写（`write = true`）。原来的文件先备份
#[tauri::command]
pub fn mcp_install(client: String, write: bool) -> CmdResult<Value> {
    let p = client_config(&client).ok_or("这个客户端要自己粘贴配置")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let mut cfg: Value = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    if !cfg.is_object() {
        return Err("那个配置文件不是 JSON 对象，没敢动它".into());
    }
    if cfg.get("mcpServers").is_none() {
        cfg["mcpServers"] = json!({});
    }
    cfg["mcpServers"]["leafstarter"] = json!({ "command": exe, "args": ["--mcp"] });
    let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    if write {
        if p.is_file() {
            std::fs::copy(&p, p.with_extension("json.leafstarter-bak")).map_err(|e| format!("备份原配置失败：{e}"))?;
        } else if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, &text).map_err(|e| format!("写不了 {}：{e}", p.display()))?;
    }
    Ok(json!({ "path": p.to_string_lossy(), "content": text, "written": write }))
}
