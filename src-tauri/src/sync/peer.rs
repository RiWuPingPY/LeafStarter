//! 局域网直传的另一头：用配对码配对、发加密请求。电脑连电脑用它；安卓端以后也用同一份代码。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::crypto;
use super::devices::{Devices, Known, Me};
use super::lan::{frame, unframe};

/// 配对码：`LS1.` + base64url(JSON { h: [地址], p: 端口, t: 口令, i: id, n: 名字 })
pub fn pair_code(hosts: &[String], port: u16, token: &[u8; 16], me: &Me) -> String {
    let j = json!({ "h": hosts, "p": port, "t": crypto::b64(token), "i": me.id, "n": me.name });
    format!("LS1.{}", crypto::b64(&serde_json::to_vec(&j).unwrap_or_default()))
}

pub struct PairInfo {
    pub hosts: Vec<String>,
    pub port: u16,
    pub token: [u8; 16],
    pub id: String,
    pub name: String,
}

pub fn parse_code(code: &str) -> Result<PairInfo> {
    let body = code.trim().strip_prefix("LS1.").context("这不是 LeafStarter 的配对码")?;
    let v: Value = serde_json::from_slice(&crypto::unb64(body)?).context("配对码不完整")?;
    let token: [u8; 16] = crypto::unb64(v["t"].as_str().unwrap_or(""))?.try_into().map_err(|_| anyhow::anyhow!("配对码不完整"))?;
    Ok(PairInfo {
        hosts: serde_json::from_value(v["h"].clone()).unwrap_or_default(),
        port: v["p"].as_u64().unwrap_or(0) as u16,
        token,
        id: v["i"].as_str().unwrap_or("").into(),
        name: v["n"].as_str().unwrap_or("另一台设备").into(),
    })
}

fn http() -> Result<reqwest::Client> {
    // 局域网直连：不走系统代理（加速器、代理软件会把局域网地址也转出去）
    Ok(reqwest::Client::builder().no_proxy().connect_timeout(Duration::from_secs(3)).timeout(Duration::from_secs(600)).build()?)
}

/// 用配对码配对：试遍码里的地址，成功就返回对方（存进 peers 由调用方做）
pub async fn pair(code: &str, me: &Me) -> Result<Known> {
    let info = parse_code(code)?;
    let k = crypto::derive("leafstarter-pair", &info.token);
    let body = crypto::seal(&k, "/pair", &frame(&json!({ "id": me.id, "name": me.name }), &[]));
    let c = http()?;
    let mut last = None;
    for h in &info.hosts {
        let url = format!("http://{}/pair", join_host(h, info.port));
        match c.post(&url).body(body.clone()).send().await {
            Ok(r) if r.status().is_success() => {
                let b = r.bytes().await?;
                let (v, _) = unframe(&crypto::open(&k, "/pair", &b)?)?;
                let key = crypto::key32(v["key"].as_str().context("对方没给密钥")?)?;
                let mut hosts = vec![h.clone()];
                hosts.extend(info.hosts.iter().filter(|x| *x != h).cloned());
                return Devices::new_known(v["id"].as_str().unwrap_or(&info.id), v["name"].as_str().unwrap_or(&info.name), &key, hosts, info.port);
            }
            Ok(r) if r.status().as_u16() == 403 => bail!("对方拒绝了：配对码过期或者已经用过，在那台设备上重新生成一个"),
            Ok(r) => last = Some(anyhow::anyhow!("对方回了 {}", r.status())),
            Err(e) => last = Some(e.into()),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("配对码里没有地址")).context("连不上对方。两台设备要连着同一个 Wi-Fi；学校、宿舍的网络常常不让设备互相连，可以改用手机热点"))
}

fn join_host(h: &str, port: u16) -> String {
    if h.contains(':') && !h.starts_with('[') {
        format!("[{h}]:{port}")
    } else {
        format!("{h}:{port}")
    }
}

/// 连到一台配对过的设备
pub struct Client {
    http: reqwest::Client,
    me: String,
    key: [u8; 32],
    hosts: Vec<String>,
    port: u16,
    /// 上次连通的地址
    good: std::sync::Mutex<Option<String>>,
}

impl Client {
    pub fn new(me: &Me, peer: &Known) -> Result<Client> {
        Ok(Client { http: http()?, me: me.id.clone(), key: peer.key()?, hosts: peer.hosts.clone(), port: peer.port, good: std::sync::Mutex::new(None) })
    }

    pub async fn call(&self, path: &str, v: Value, bytes: &[u8]) -> Result<(Value, Vec<u8>)> {
        let body = crypto::seal(&self.key, path, &frame(&v, bytes));
        let good = self.good.lock().unwrap().clone();
        let mut order: Vec<String> = good.iter().cloned().collect();
        order.extend(self.hosts.iter().filter(|h| Some(*h) != good.as_ref()).cloned());
        let mut last = None;
        for h in order {
            let url = format!("http://{}{}", join_host(&h, self.port), path);
            match self.http.post(&url).header("x-ls-device", &self.me).body(body.clone()).send().await {
                Ok(r) if r.status().as_u16() == 403 => bail!("对方不认这台设备了（可能把配对删了），重新配对一次"),
                Ok(r) => {
                    let b = r.error_for_status()?.bytes().await?;
                    *self.good.lock().unwrap() = Some(h);
                    let (v, data) = unframe(&crypto::open(&self.key, path, &b)?)?;
                    if let Some(e) = v.get("error").and_then(Value::as_str) {
                        bail!("{e}");
                    }
                    return Ok((v, data));
                }
                Err(e) => last = Some(e),
            }
        }
        Err(anyhow::anyhow!("连不上对方：{}", last.map(|e| e.to_string()).unwrap_or_default())).context("对方的 LeafStarter 开着「允许局域网连接」吗？两台设备要在同一个网络里")
    }
}
