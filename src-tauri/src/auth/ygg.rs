//! 外置登录（authlib-injector / Yggdrasil 协议，ROADMAP 5.4）：LittleSkin 这类皮肤站。
//!
//! - 填的地址可以是网站首页：按 ALI（`X-Authlib-Injector-API-Location` 响应头）找到真正的 API 地址
//! - 密码只用来换一次令牌，不保存；令牌加密后落盘，启动前先校验、过期了刷新
//! - 启动时加 `-javaagent:authlib-injector.jar=<API 地址>` 和预取的服务器元数据（省得游戏启动时再请求一次）

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

/// 找到的认证服务器
#[derive(Debug, Clone)]
pub struct Server {
    /// API 地址，结尾带 `/`
    pub api: String,
    /// 服务器名字（元数据里的 meta.serverName）
    pub name: String,
}

/// 用户填的地址 → API 地址和元数据
pub async fn discover(http: &reqwest::Client, input: &str) -> Result<Server> {
    let input = input.trim();
    if input.is_empty() {
        bail!("填一下认证服务器的地址，比如 littleskin.cn");
    }
    // 没写 http:// 还是 https://：先试 https，连不上再试 http（自己在局域网搭的皮肤站常常只有 http）
    let tries: Vec<String> = if input.contains("://") { vec![input.to_string()] } else { vec![format!("https://{input}"), format!("http://{input}")] };
    let mut last = None;
    let mut found = None;
    for url in tries {
        let parsed = reqwest::Url::parse(&url).context("认证服务器的地址不对")?;
        if !matches!(parsed.scheme(), "https" | "http") {
            bail!("认证服务器的地址要以 https:// 开头");
        }
        match http.get(parsed.clone()).send().await {
            Ok(res) => {
                found = Some((parsed, res));
                break;
            }
            Err(e) => last = Some(e),
        }
    }
    let Some((parsed, res)) = found else {
        bail!("连不上认证服务器：{}", last.map(|e| if e.is_timeout() { "超时了".to_string() } else if e.is_connect() { "连接失败，检查一下地址".to_string() } else { e.to_string() }).unwrap_or_default());
    };
    // ALI：网站首页告诉我们 API 在哪
    let api = match res.headers().get("x-authlib-injector-api-location").and_then(|v| v.to_str().ok()) {
        Some(loc) => parsed.join(loc).context("认证服务器给的 API 地址不对")?,
        None => parsed,
    };
    let mut api = api.to_string();
    if !api.ends_with('/') {
        api.push('/');
    }
    let meta = http.get(&api).send().await.context("连不上认证服务器")?.text().await?;
    let v: Value = serde_json::from_str(&meta).map_err(|_| anyhow::anyhow!("这个地址不是外置登录（authlib-injector）的认证服务器"))?;
    if v.get("meta").is_none() && v.get("skinDomains").is_none() {
        bail!("这个地址不是外置登录（authlib-injector）的认证服务器");
    }
    let name = v.pointer("/meta/serverName").and_then(Value::as_str).unwrap_or("外置登录").to_string();
    Ok(Server { api, name })
}

/// 服务器返回的错误换成中文
fn why(status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let msg = v.get("errorMessage").and_then(Value::as_str).unwrap_or("");
    let lower = msg.to_lowercase();
    if lower.contains("invalid credentials") || lower.contains("password") {
        "账号或密码不对".into()
    } else if lower.contains("invalid token") {
        "登录过期了，重新登录一次".into()
    } else if status == 429 || lower.contains("too many") {
        "试得太频繁了，等一会儿再登录".into()
    } else if msg.is_empty() {
        format!("认证服务器返回 {status}")
    } else {
        format!("认证服务器说：{msg}")
    }
}

async fn post(http: &reqwest::Client, url: String, body: Value) -> Result<(u16, String)> {
    let res = http.post(url).json(&body).send().await.context("连不上认证服务器")?;
    let status = res.status().as_u16();
    Ok((status, res.text().await.unwrap_or_default()))
}

/// 登录结果：令牌、全部角色、已选的角色（只有一个角色时服务器会直接选上）
pub struct Session {
    pub access: String,
    pub client: String,
    pub profiles: Vec<Profile>,
    pub selected: Option<Profile>,
}

fn session(body: &str, client: &str) -> Result<Session> {
    let v: Value = serde_json::from_str(body).context("认证服务器的回复看不懂")?;
    let access = v.get("accessToken").and_then(Value::as_str).context("认证服务器没给令牌")?.to_string();
    let profiles: Vec<Profile> = v.get("availableProfiles").and_then(|p| serde_json::from_value(p.clone()).ok()).unwrap_or_default();
    let selected: Option<Profile> = v.get("selectedProfile").and_then(|p| serde_json::from_value(p.clone()).ok());
    let client = v.get("clientToken").and_then(Value::as_str).unwrap_or(client).to_string();
    Ok(Session { access, client, profiles, selected })
}

pub async fn authenticate(http: &reqwest::Client, api: &str, username: &str, password: &str) -> Result<Session> {
    let client = random_token();
    let body = json!({ "agent": { "name": "Minecraft", "version": 1 }, "username": username.trim(), "password": password, "clientToken": client, "requestUser": false });
    let (status, text) = post(http, format!("{api}authserver/authenticate"), body).await?;
    if status != 200 {
        bail!("{}", why(status, &text));
    }
    let s = session(&text, &client)?;
    if s.profiles.is_empty() && s.selected.is_none() {
        bail!("这个账号下还没有角色，先去皮肤站建一个");
    }
    Ok(s)
}

/// 刷新令牌；`pick` 是多角色时选中的那个
pub async fn refresh(http: &reqwest::Client, api: &str, access: &str, client: &str, pick: Option<&Profile>) -> Result<Session> {
    let mut body = json!({ "accessToken": access, "clientToken": client, "requestUser": false });
    if let Some(p) = pick {
        body["selectedProfile"] = json!({ "id": p.id, "name": p.name });
    }
    let (status, text) = post(http, format!("{api}authserver/refresh"), body).await?;
    if status != 200 {
        bail!("{}", why(status, &text));
    }
    session(&text, client)
}

/// 令牌还能不能用（204 就是能用）
pub async fn validate(http: &reqwest::Client, api: &str, access: &str, client: &str) -> bool {
    matches!(post(http, format!("{api}authserver/validate"), json!({ "accessToken": access, "clientToken": client })).await, Ok((204, _)))
}

fn random_token() -> String {
    let mut b = [0u8; 16];
    getrandom(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 不为一个随机数多拉一个依赖：用时间、进程号、栈地址做 SHA-256
fn getrandom(out: &mut [u8; 16]) {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let local = 0u8;
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let h = Sha256::new().chain_update(t.to_le_bytes()).chain_update(n.to_le_bytes()).chain_update(std::process::id().to_le_bytes()).chain_update((&local as *const u8 as usize).to_le_bytes()).finalize();
    out.copy_from_slice(&h[..16]);
}

// ---------- authlib-injector 本体 ----------

const LATEST: &[&str] = &[
    "https://authlib-injector.yushi.moe/artifact/latest.json",
    "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json",
];

#[derive(Deserialize)]
struct Latest {
    version: String,
    download_url: String,
    checksums: Checksums,
}

#[derive(Deserialize)]
struct Checksums {
    sha256: String,
}

/// 拿到 authlib-injector.jar：查最新版本，没下过就下载并校验 SHA-256；查不到最新版本时用以前下好的
pub async fn ensure_agent(data_dir: &Path) -> Result<PathBuf> {
    let dir = data_dir.join("authlib-injector");
    std::fs::create_dir_all(&dir)?;
    let http = client()?;
    let mut last_err = None;
    for url in LATEST {
        let latest: Latest = match async { anyhow::Ok(http.get(*url).send().await?.error_for_status()?.json().await?) }.await {
            Ok(l) => l,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let jar = dir.join(format!("authlib-injector-{}.jar", latest.version.replace(['/', '\\'], "")));
        if jar.is_file() {
            return Ok(jar);
        }
        match async { anyhow::Ok(http.get(&latest.download_url).send().await?.error_for_status()?.bytes().await?) }.await {
            Ok(b) => {
                let got: String = Sha256::digest(&b).iter().map(|x| format!("{x:02x}")).collect();
                if !got.eq_ignore_ascii_case(latest.checksums.sha256.trim()) {
                    last_err = Some(anyhow::anyhow!("下载的 authlib-injector 校验不对"));
                    continue;
                }
                std::fs::write(&jar, &b)?;
                return Ok(jar);
            }
            Err(e) => last_err = Some(e),
        }
    }
    // 联不上网：用以前下好的最新一个
    let mut cached: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jar")).collect();
    cached.sort_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());
    cached.pop().ok_or_else(|| anyhow::anyhow!("下载 authlib-injector 失败：{:#}", last_err.unwrap_or_else(|| anyhow::anyhow!("没有可用的下载地址"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_in_chinese() {
        assert_eq!(why(403, r#"{"error":"ForbiddenOperationException","errorMessage":"Invalid credentials. Invalid username or password."}"#), "账号或密码不对");
        assert_eq!(why(403, r#"{"errorMessage":"Invalid token."}"#), "登录过期了，重新登录一次");
        assert_eq!(why(500, ""), "认证服务器返回 500");
    }

    #[test]
    fn tokens_differ() {
        assert_ne!(random_token(), random_token());
        assert_eq!(random_token().len(), 32);
    }
}
