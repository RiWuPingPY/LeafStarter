//! 对外 MCP 服务端（ROADMAP 5.13）：让用户已经在用的 AI 客户端（Claude 桌面版、Claude Code、Cursor……）操作 LeafStarter。
//!
//! - 启动器开着「开放 MCP」时，在 127.0.0.1:47816（被占了就随机端口）听 JSON-RPC（`POST /mcp`，Streamable HTTP），
//!   要带令牌（Authorization: Bearer）。令牌第一次生成后存在 `<数据目录>/mcp-token`，以后不变，
//!   支持 HTTP 的客户端配一次就行。端口和令牌也写进 `<数据目录>/mcp.json` 给 stdio 入口用
//! - 带 Origin 头的请求只收本机网页来的（防 DNS 重绑定：别的网站的脚本不能借浏览器打进来）
//! - `leafstarter --mcp` 是给客户端用的 stdio 入口：一行一条 JSON-RPC，转给正在运行的启动器
//! - 只读和可撤回两级按设置放行；影响外部的（启动游戏、结束游戏）每次都在 LeafStarter 自己的窗口里弹确认

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::tools::{self, Level};
use crate::state::AppState;

pub const PROTOCOL: &str = "2025-06-18";

pub struct Server {
    pub port: u16,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}

fn info_path(data_dir: &Path) -> PathBuf {
    data_dir.join("mcp.json")
}

/// 固定端口：支持 HTTP 的客户端配一次就不用再改。被别的程序占了就用随机端口
pub const PORT: u16 = 47816;

/// 令牌第一次生成后就一直用这个（存在数据目录，只有本机这个用户读得到）
pub fn token(data_dir: &Path) -> String {
    let p = data_dir.join("mcp-token");
    if let Ok(t) = std::fs::read_to_string(&p) {
        if t.trim().len() >= 16 {
            return t.trim().to_string();
        }
    }
    let t = crate::sync::crypto::b64(&crate::sync::crypto::random::<24>()).replace(['+', '/', '='], "");
    let _ = std::fs::write(&p, &t);
    t
}

/// 开始听；端口和令牌写进 mcp.json（只有本机这个用户读得到）
pub async fn start(app: AppHandle) -> Result<Server> {
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind("127.0.0.1:0").await?,
    };
    let port = listener.local_addr()?.port();
    let data_dir = app.state::<AppState>().data_dir.clone();
    let token = token(&data_dir);
    std::fs::write(info_path(&data_dir), serde_json::to_vec(&json!({ "port": port, "token": token, "pid": std::process::id() }))?)?;
    let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
    let token = Arc::new(token);
    tokio::spawn(async move {
        loop {
            let (sock, _) = tokio::select! {
                r = listener.accept() => match r { Ok(x) => x, Err(_) => continue },
                _ = &mut stopped => break,
            };
            let (app, token) = (app.clone(), token.clone());
            tokio::spawn(async move {
                let svc = hyper::service::service_fn(move |req| handle(app.clone(), token.clone(), req));
                let _ = hyper::server::conn::http1::Builder::new().serve_connection(hyper_util::rt::TokioIo::new(sock), svc).await;
            });
        }
        let _ = std::fs::remove_file(info_path(&data_dir));
    });
    Ok(Server { port, stop: Some(stop) })
}

async fn handle(app: AppHandle, token: Arc<String>, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let ok_auth = req.headers().get("authorization").and_then(|v| v.to_str().ok()) == Some(&format!("Bearer {token}"));
    // 浏览器里别的网站发来的（带 Origin，又不是本机）一律不收
    let origin_ok = req.headers().get("origin").and_then(|v| v.to_str().ok()).is_none_or(|o| {
        let o = o.trim_end_matches('/');
        o == "null" || ["http://127.0.0.1", "http://localhost", "https://localhost"].iter().any(|p| o == *p || o.starts_with(&format!("{p}:")))
    });
    if !origin_ok {
        return Ok(Response::builder().status(StatusCode::FORBIDDEN).body(Full::new(Bytes::new())).unwrap());
    }
    // Streamable HTTP 的 GET（服务端推送）用不上：明确说不支持
    if req.method() == hyper::Method::GET && req.uri().path() == "/mcp" && ok_auth {
        return Ok(Response::builder().status(StatusCode::METHOD_NOT_ALLOWED).header("allow", "POST").body(Full::new(Bytes::new())).unwrap());
    }
    if req.uri().path() != "/mcp" || !ok_auth {
        return Ok(Response::builder().status(StatusCode::UNAUTHORIZED).body(Full::new(Bytes::new())).unwrap());
    }
    let body = req.into_body().collect().await.map(|b| b.to_bytes()).unwrap_or_default();
    let out = match serde_json::from_slice::<Value>(&body) {
        Ok(msg) => rpc(&app, msg).await,
        Err(_) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } })),
    };
    let (status, bytes) = match out {
        Some(v) => (StatusCode::OK, serde_json::to_vec(&v).unwrap_or_default()),
        None => (StatusCode::ACCEPTED, vec![]),
    };
    Ok(Response::builder().status(status).header("content-type", "application/json").body(Full::new(Bytes::from(bytes))).unwrap())
}

/// 处理一条 JSON-RPC；通知（没有 id）返回 None
pub async fn rpc(app: &AppHandle, msg: Value) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let method = msg["method"].as_str().unwrap_or("");
    let res: Result<Value, (i64, String)> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "leafstarter", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "LeafStarter 是一个 Minecraft Java 版启动器。可以列出实例、看日志和崩溃报告、开关 Mod、改内存和 Java、从 Modrinth 装 Mod、启动和结束游戏。启动、结束游戏要用户在 LeafStarter 窗口里确认。",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => {
            let levels = allowed(app);
            Ok(json!({ "tools": tools::all().into_iter().filter(|t| t.level == Level::Ext || levels.contains(&t.level)).map(|t| json!({
                "name": t.spec.name, "title": t.title, "description": t.spec.description, "inputSchema": t.spec.schema,
                "annotations": { "readOnlyHint": t.level == Level::Read, "destructiveHint": false, "openWorldHint": t.level == Level::Ext },
            })).collect::<Vec<_>>() }))
        }
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            let input = msg["params"]["arguments"].clone();
            let input = if input.is_null() { json!({}) } else { input };
            Ok(call(app, name, &input).await)
        }
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match res {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, m)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": m } }),
    })
}

fn allowed(app: &AppHandle) -> Vec<Level> {
    let s = app.state::<AppState>().settings.lock().unwrap().ai.mcp_levels.clone();
    let s = if s.is_empty() { vec!["read".to_string(), "rev".to_string()] } else { s };
    s.iter()
        .filter_map(|x| match x.as_str() {
            "read" => Some(Level::Read),
            "rev" => Some(Level::Rev),
            _ => None,
        })
        .collect()
}

async fn call(app: &AppHandle, name: &str, input: &Value) -> Value {
    let text = |t: String, err: bool| json!({ "content": [{ "type": "text", "text": t }], "isError": err });
    let Some(tool) = tools::find(name).filter(|t| t.spec.name != "remove_files") else { return text(format!("没有这个工具：{name}"), true) };
    if tool.level != Level::Ext && !allowed(app).contains(&tool.level) {
        return text("LeafStarter 设置里没有开放这一级的操作".into(), true);
    }
    // 影响外部的：在 LeafStarter 自己的窗口里问，外部客户端代答不了
    if tool.level == Level::Ext {
        let ans = super::confirm(app, "mcp", &tool, input).await;
        if ans == "no" {
            return text("用户在 LeafStarter 里没有允许这个操作".into(), true);
        }
    }
    {
        use tauri::Emitter;
        // 面板上记一笔：外部客户端调了什么
        let _ = app.emit("agent-event", json!({ "session": "mcp", "kind": "mcp", "tool": name, "title": tool.title, "input": input }));
    }
    match tools::run(app, name, input).await {
        Ok((out, _)) => text(out, false),
        Err(e) => text(format!("出错了：{e:#}"), true),
    }
}

// ---------- `leafstarter --mcp`：stdio 入口 ----------

/// 找正在运行的启动器写的 mcp.json
fn find_info() -> Option<Value> {
    let base = dirs::data_dir()?;
    let p = base.join("com.leafhub.leafstarter").join("mcp.json");
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// 一行一条 JSON-RPC，从标准输入读，转给启动器，结果写到标准输出
pub fn stdio() -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(r) => r,
        Err(_) => return 1,
    };
    let http = reqwest::Client::builder().no_proxy().build().expect("http");
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let id = msg.get("id").cloned();
        let reply = rt.block_on(async {
            let info = find_info().context("LeafStarter 没开着，或者没有打开「开放 MCP」")?;
            let url = format!("http://127.0.0.1:{}/mcp", info["port"]);
            let r = http.post(url).bearer_auth(info["token"].as_str().unwrap_or("")).json(&msg).send().await.context("连不上 LeafStarter")?;
            if r.status() == 202 {
                return anyhow::Ok(None);
            }
            Ok(Some(r.json::<Value>().await?))
        });
        let v = match reply {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(e) => match id {
                Some(id) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": format!("{e:#}") } }),
                None => continue,
            },
        };
        let _ = writeln!(out, "{}", v);
        let _ = out.flush();
    }
    0
}
