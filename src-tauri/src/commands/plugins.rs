//! 插件（M15–M17）：列表、装（文件 / 市场）、开关、卸载、开发者模式、脚本插件要的文件 / 联网 / 存储、原生插件调用

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::plugin::{host, sign};
use crate::state::{err, AppState, CmdResult};

fn dd(st: &AppState) -> PathBuf {
    st.data_dir.clone()
}

#[tauri::command]
pub fn plugin_list(state: State<'_, AppState>) -> Value {
    let st = host::load_state(&dd(&state));
    let contrib = host::contributions(&dd(&state), state.safe_mode);
    // 插件提供的镜像源接进下载源（界面每次插件有变化都会重新来要这份列表）
    state.net.set_plugin_mirrors(contrib["mirrors"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    crate::agent::tools::set_plugin_tools(contrib["scripts"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    json!({
        "plugins": host::list(&dd(&state), state.safe_mode),
        "safe": state.safe_mode, "dev": st.dev, "devDir": st.dev_dir,
        "contrib": contrib,
    })
}

/// 装之前先看看：要什么权限、谁签的、是不是升级（升级时多要了哪些权限）
#[tauri::command]
pub fn plugin_inspect(state: State<'_, AppState>, path: String) -> CmdResult<Value> {
    let (m, key, sha, _) = host::inspect(Path::new(&path)).map_err(err)?;
    let st = host::load_state(&dd(&state));
    let old = st.plugins.get(&m.id);
    let new_perms: Vec<&String> = m.permissions.iter().filter(|p| old.is_none_or(|o| !o.granted.contains(p))).collect();
    Ok(json!({
        "manifest": m, "fingerprint": sign::fingerprint(&key), "sha256": sha,
        "perms": m.permissions.iter().map(|p| json!({ "id": p, "text": host::perm_text(p), "new": new_perms.contains(&p) })).collect::<Vec<_>>(),
        "upgrade": old.map(|o| json!({ "from": o.version, "sameKey": o.key == sign::fingerprint(&key) })),
        "revoked": st.revoked.get(&m.id),
    }))
}

#[tauri::command]
pub fn plugin_install(state: State<'_, AppState>, path: String, granted: Vec<String>) -> CmdResult<Value> {
    let m = host::install(&dd(&state), Path::new(&path), &granted, "file", "community").map_err(err)?;
    Ok(json!(m))
}

#[tauri::command]
pub async fn plugin_pick(app: AppHandle, folder: Option<bool>) -> CmdResult<Option<String>> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let send = move |f: Option<tauri_plugin_dialog::FilePath>| {
            let _ = tx.send(f.map(|p| p.to_string()));
        };
        if folder.unwrap_or(false) {
            app.dialog().file().set_title("选插件的文件夹（里面有 plugin.json）").pick_folder(send);
        } else {
            app.dialog().file().set_title("选插件包").add_filter("LeafStarter 插件", &[host::EXT]).pick_file(send);
        }
        Ok(rx.await.ok().flatten())
    }
    #[cfg(target_os = "android")]
    {
        let _ = (app, folder);
        Ok(None)
    }
}

#[tauri::command]
pub fn plugin_enable(state: State<'_, AppState>, id: String, on: bool) -> CmdResult<()> {
    host::update_state(&dd(&state), |s| {
        if on && s.revoked.contains_key(&id) {
            anyhow::bail!("这个插件被吊销了，不能再开");
        }
        let r = s.plugins.get_mut(&id).ok_or_else(|| anyhow::anyhow!("没有这个插件"))?;
        r.enabled = on;
        Ok(())
    })
    .map_err(err)
}

#[tauri::command]
pub async fn plugin_remove(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.native.stop(&id).await;
    host::remove(&dd(&state), &id).map_err(err)
}

/// 插件市场的索引（同时按吊销名单停用）
#[tauri::command]
pub async fn plugin_index(state: State<'_, AppState>) -> CmdResult<Value> {
    let url = std::env::var("LEAFSTARTER_PLUGIN_INDEX").unwrap_or_else(|_| host::INDEX_URL.to_string());
    host::fetch_index(&dd(&state), &url).await.map_err(err)
}

#[derive(Deserialize)]
pub struct MarketEntry {
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub sig: String,
}

#[tauri::command]
pub async fn plugin_market_install(state: State<'_, AppState>, entry: MarketEntry, granted: Vec<String>) -> CmdResult<Value> {
    let p = host::download(&dd(&state), &entry.url, &entry.sha256).await.map_err(err)?;
    // 档位按官方根密钥的签名重新判一次：界面传上来的不算数
    let tier = host::market_tier(&json!({ "id": entry.id, "version": entry.version, "sha256": entry.sha256, "tier": entry.tier, "sig": entry.sig }));
    let (m0, ..) = host::inspect(&p).map_err(err)?;
    if !entry.id.is_empty() && m0.id != entry.id {
        return Err(format!("下下来的插件是「{}」，不是市场里写的「{}」", m0.id, entry.id));
    }
    let m = host::install(&dd(&state), &p, &granted, "market", &tier).map_err(err)?;
    Ok(json!(m))
}

/// 下载市场里的包，只看不装（给用户看权限）
#[tauri::command]
pub async fn plugin_market_inspect(state: State<'_, AppState>, entry: MarketEntry) -> CmdResult<Value> {
    let p = host::download(&dd(&state), &entry.url, &entry.sha256).await.map_err(err)?;
    plugin_inspect(state, p.to_string_lossy().into_owned())
}

/// 开发者模式：从文件夹加载一个本地插件（不用签名，标「本地开发」）
#[tauri::command]
pub fn plugin_dev(state: State<'_, AppState>, on: bool, dir: Option<String>) -> CmdResult<()> {
    host::update_state(&dd(&state), |s| {
        s.dev = on;
        if let Some(d) = dir {
            s.dev_dir = d;
        }
        Ok(())
    })
    .map_err(err)
}

#[tauri::command]
pub fn plugin_file(state: State<'_, AppState>, id: String, name: String) -> CmdResult<String> {
    host::read_file(&dd(&state), &id, &name).map_err(err)
}

fn perms_of(st: &AppState, id: &str) -> CmdResult<Vec<String>> {
    host::enabled(&dd(st), st.safe_mode).into_iter().find(|(m, _, _)| m.id == id).map(|(m, _, _)| m.permissions).ok_or_else(|| "这个插件没开着".to_string())
}

/// 脚本插件联网：只能 GET 它在权限里写了的域名
#[tauri::command]
pub async fn plugin_fetch(state: State<'_, AppState>, id: String, url: String) -> CmdResult<Value> {
    let perms = perms_of(&state, &id)?;
    let u = reqwest::Url::parse(&url).map_err(|_| "网址不对")?;
    if u.scheme() != "https" {
        return Err("只能访问 https 网址".into());
    }
    let allowed: Vec<String> = perms.iter().filter_map(|p| p.strip_prefix("net:").map(String::from)).collect();
    let ok = move |host: &str| allowed.iter().any(|h| h == host || (h.starts_with("*.") && host.ends_with(&h[1..])));
    let host = u.host_str().unwrap_or("").to_string();
    if !ok(&host) {
        return Err(format!("插件没申请访问 {host} 的权限"));
    }
    // 跳转也只能跳到申请过的 https 域名，不然申请一个域名就能借它的跳转访问任何地方
    let policy = reqwest::redirect::Policy::custom(move |a| {
        let fine = a.url().scheme() == "https" && ok(a.url().host_str().unwrap_or(""));
        if !fine || a.previous().len() >= 5 {
            a.stop()
        } else {
            a.follow()
        }
    });
    let client = reqwest::Client::builder().user_agent(concat!("LeafStarter-plugin/", env!("CARGO_PKG_VERSION"))).timeout(std::time::Duration::from_secs(20)).redirect(policy).build().map_err(|e| e.to_string())?;
    let r = client.get(u).send().await.map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    // 边读边数，超过 2 MB 就停，不先整个读进内存
    let mut b: Vec<u8> = vec![];
    let mut stream = r.bytes_stream();
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        b.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        if b.len() > 2 * 1024 * 1024 {
            return Err("返回的内容太大了".into());
        }
    }
    Ok(json!({ "status": status, "text": String::from_utf8_lossy(&b) }))
}

/// 脚本插件的本地存储：一个插件一个 JSON 文件，最多 256 KB
#[tauri::command]
pub fn plugin_storage(state: State<'_, AppState>, id: String, key: String, value: Option<Value>) -> CmdResult<Value> {
    if !perms_of(&state, &id)?.iter().any(|p| p == "storage") {
        return Err("插件没申请存储权限".into());
    }
    if id.contains(['/', '\\', '.']) || key.len() > 100 {
        return Err("参数不对".into());
    }
    let p = host::root(&dd(&state)).join("storage").join(format!("{id}.json"));
    let mut all: serde_json::Map<String, Value> = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match value {
        None => Ok(all.get(&key).cloned().unwrap_or(Value::Null)),
        Some(v) => {
            all.insert(key, v);
            let b = serde_json::to_vec(&all).map_err(|e| e.to_string())?;
            if b.len() > 256 * 1024 {
                return Err("插件存的东西太多了（最多 256 KB）".into());
            }
            std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&p, b).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
    }
}

/// 调原生插件自带的程序
#[tauri::command]
pub async fn plugin_native(state: State<'_, AppState>, id: String, method: String, params: Value) -> CmdResult<Value> {
    if !perms_of(&state, &id)?.iter().any(|p| p == "native") {
        return Err("插件没申请 native 权限".into());
    }
    state.native.call(&dd(&state), state.safe_mode, &id, &method, params).await.map_err(err)
}

/// 插件页面交回 AI 工具的结果
#[tauri::command]
pub fn plugin_tool_result(call: String, result: Option<Value>, error: Option<String>) -> bool {
    crate::agent::tools::plugin_tool_done(&call, match error {
        Some(e) => Err(e.chars().take(500).collect()),
        None => Ok(result.unwrap_or(Value::Null)),
    })
}
