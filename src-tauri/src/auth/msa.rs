//! 微软正版登录：设备代码 → 微软令牌 → Xbox Live → XSTS → Minecraft 令牌 → 查拥有和档案。
//! 客户端 ID 是我们在 Azure 上注册的公开客户端（见 ROADMAP D5），不是密钥。

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

pub const CLIENT_ID: &str = "99d38c69-86e4-403a-bd64-8a12d14ac42e";
const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    #[serde(skip)]
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub struct MsToken {
    pub access: String,
    pub refresh: String,
}

pub enum Poll {
    Pending,
    SlowDown,
    Done(MsToken),
}

pub struct McLogin {
    pub access_token: String,
    pub expires_in: u64,
    pub uuid: String,
    pub name: String,
}

pub fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?)
}

async fn json_of(r: reqwest::Response) -> Result<(u16, Value)> {
    let status = r.status().as_u16();
    let v = r.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, v))
}

pub async fn device_code(http: &reqwest::Client) -> Result<DeviceCode> {
    let r = http.post(format!("{}/devicecode", AUTHORITY)).form(&[("client_id", CLIENT_ID), ("scope", SCOPE)]).send().await.context("连不上微软登录服务器")?;
    let (status, v) = json_of(r).await?;
    if status != 200 {
        bail!("微软登录服务器拒绝了请求：{}", v.get("error_description").and_then(|e| e.as_str()).unwrap_or("未知原因"));
    }
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let n = |k: &str, d: u64| v.get(k).and_then(|x| x.as_u64()).unwrap_or(d);
    Ok(DeviceCode { device_code: s("device_code"), user_code: s("user_code"), verification_uri: s("verification_uri"), expires_in: n("expires_in", 900), interval: n("interval", 5) })
}

pub async fn poll(http: &reqwest::Client, device_code: &str) -> Result<Poll> {
    let r = http
        .post(format!("{}/token", AUTHORITY))
        .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", CLIENT_ID), ("device_code", device_code)])
        .send()
        .await
        .context("连不上微软登录服务器")?;
    let (status, v) = json_of(r).await?;
    if status == 200 {
        return Ok(Poll::Done(token_of(&v)?));
    }
    match v.get("error").and_then(|e| e.as_str()).unwrap_or("") {
        "authorization_pending" => Ok(Poll::Pending),
        "slow_down" => Ok(Poll::SlowDown),
        "expired_token" => bail!("代码过期了，重新点一次登录"),
        "authorization_declined" | "access_denied" => bail!("你在网页上拒绝了登录"),
        e => bail!("登录没成功：{}", if e.is_empty() { "未知原因" } else { e }),
    }
}

pub async fn refresh(http: &reqwest::Client, refresh_token: &str) -> Result<MsToken> {
    let r = http
        .post(format!("{}/token", AUTHORITY))
        .form(&[("grant_type", "refresh_token"), ("client_id", CLIENT_ID), ("refresh_token", refresh_token), ("scope", SCOPE)])
        .send()
        .await
        .context("连不上微软登录服务器")?;
    let (status, v) = json_of(r).await?;
    if status != 200 {
        bail!("微软账户的登录过期了，重新登录一次");
    }
    token_of(&v)
}

fn token_of(v: &Value) -> Result<MsToken> {
    let access = v.get("access_token").and_then(|x| x.as_str()).ok_or_else(|| anyhow!("微软没有给登录令牌"))?.to_string();
    let refresh = v.get("refresh_token").and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok(MsToken { access, refresh })
}

/// XSTS 常见的错误码，换成能看懂的话
fn xsts_error(code: u64) -> &'static str {
    match code {
        2148916233 => "这个微软账户还没有 Xbox 档案。先去 minecraft.net 用它登录一次，创建档案后再回来",
        2148916235 => "Xbox Live 在你所在的国家或地区不可用",
        2148916236 | 2148916237 => "这个账户需要先在 Xbox 网站上完成成年验证",
        2148916238 => "这是儿童账户，需要家长把它加入 Microsoft 家庭组后才能玩",
        _ => "Xbox Live 拒绝了登录",
    }
}

pub async fn minecraft(http: &reqwest::Client, ms_access: &str) -> Result<McLogin> {
    // Xbox Live
    let r = http
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({ "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", ms_access) }, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT" }))
        .send()
        .await
        .context("连不上 Xbox Live")?;
    let (status, xbl) = json_of(r).await?;
    if status != 200 {
        bail!("Xbox Live 登录失败（{}）", status);
    }
    let xbl_token = xbl.get("Token").and_then(|t| t.as_str()).ok_or_else(|| anyhow!("Xbox Live 没给令牌"))?;
    let uhs = xbl.pointer("/DisplayClaims/xui/0/uhs").and_then(|u| u.as_str()).ok_or_else(|| anyhow!("Xbox Live 的回应缺少用户信息"))?;

    // XSTS
    let r = http
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({ "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] }, "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT" }))
        .send()
        .await
        .context("连不上 Xbox Live")?;
    let (status, xsts) = json_of(r).await?;
    if status == 401 {
        bail!("{}", xsts_error(xsts.get("XErr").and_then(|e| e.as_u64()).unwrap_or(0)));
    }
    if status != 200 {
        bail!("Xbox Live 授权失败（{}）", status);
    }
    let xsts_token = xsts.get("Token").and_then(|t| t.as_str()).ok_or_else(|| anyhow!("Xbox Live 没给授权令牌"))?;

    // Minecraft
    let r = http
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }))
        .send()
        .await
        .context("连不上 Minecraft 服务器")?;
    let (status, mc) = json_of(r).await?;
    if status != 200 {
        bail!("Minecraft 登录失败（{}）。如果刚换过网络，过一会儿再试", status);
    }
    let access_token = mc.get("access_token").and_then(|t| t.as_str()).ok_or_else(|| anyhow!("Minecraft 没给令牌"))?.to_string();
    let expires_in = mc.get("expires_in").and_then(|e| e.as_u64()).unwrap_or(86400);

    // 档案：没买游戏或者还没起名字，这里会是 404
    let r = http.get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&access_token).send().await.context("连不上 Minecraft 服务器")?;
    let (status, profile) = json_of(r).await?;
    if status == 404 {
        bail!("这个微软账户还没有 Minecraft Java 版。买过的话，先去 minecraft.net 设置一个游戏名");
    }
    if status != 200 {
        bail!("读不到游戏档案（{}）", status);
    }
    let uuid = profile.get("id").and_then(|i| i.as_str()).ok_or_else(|| anyhow!("档案里没有 UUID"))?.to_string();
    let name = profile.get("name").and_then(|n| n.as_str()).ok_or_else(|| anyhow!("档案里没有游戏名"))?.to_string();
    Ok(McLogin { access_token, expires_in, uuid, name })
}
