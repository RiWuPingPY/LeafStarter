//! 局域网直传（ROADMAP 5.7 默认那条路，不经过我们的任何服务器）。
//!
//! 一台设备开着「允许局域网连接」时在 38765 端口（被占就随便一个）听 HTTP；只接受局域网地址来的请求。
//! 配对：界面上显示配对码 / 二维码（地址、端口、一次性口令）。对方拿口令派生的密钥发 `/pair`，
//! 这边给它发一把只属于它的密钥。之后每个请求带 `x-ls-device: <它的 id>`，正文用它的密钥加密，
//! 附加数据是路径；解得开才处理。
//!
//! 正文明文的格式：4 字节长度（大端）+ JSON + 可选的二进制（文件内容）。

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::crypto;
use super::devices::Devices;
use super::store::{Files, Store};
use crate::base::util::now_ms;

pub const PORT: u16 = 38765;
const PAIR_TTL_MS: u64 = 10 * 60 * 1000;

// ---------- 帧 ----------

pub fn frame(v: &Value, bytes: &[u8]) -> Vec<u8> {
    let j = serde_json::to_vec(v).unwrap_or_default();
    let mut out = Vec::with_capacity(4 + j.len() + bytes.len());
    out.extend_from_slice(&(j.len() as u32).to_be_bytes());
    out.extend(j);
    out.extend_from_slice(bytes);
    out
}

pub fn unframe(b: &[u8]) -> Result<(Value, Vec<u8>)> {
    if b.len() < 4 {
        bail!("数据不完整");
    }
    let n = u32::from_be_bytes(b[..4].try_into()?) as usize;
    if b.len() < 4 + n {
        bail!("数据不完整");
    }
    Ok((serde_json::from_slice(&b[4..4 + n])?, b[4 + n..].to_vec()))
}

// ---------- 对外提供什么 ----------

/// 可以同步的实例（给对方看的）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInst {
    /// 这边的实例 key（游戏目录 id/版本名）
    pub key: String,
    pub name: String,
    pub mc: Option<String>,
    pub loader: String,
    pub loader_version: Option<String>,
    pub launcher: String,
    pub worlds: Vec<RemoteWorld>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorld {
    pub folder: String,
    pub modified: u64,
}

/// 服务端从宿主那里拿实例信息（真程序里是扫描结果；测试里是假的）
pub trait Host: Send + Sync + 'static {
    fn instances(&self) -> Vec<RemoteInst>;
    /// 实例 key → 游戏实际运行的目录
    fn game_dir(&self, key: &str) -> Option<PathBuf>;
    fn running(&self, key: &str) -> bool;
    fn changed(&self, _what: &str) {}
}

/// 实例里要同步的文件夹和文件（世界单独同步）
pub const INSTANCE_DIRS: &[&str] = &["mods", "config", "defaultconfigs", "kubejs", "scripts", "resourcepacks", "shaderpacks"];
pub const INSTANCE_FILES: &[&str] = &["options.txt", "optionsof.txt", "optionsshaders.txt", "servers.dat"];

/// 实例的文件清单（不含存档），路径带上文件夹前缀，比如 `mods/sodium.jar`
pub fn instance_files(store: &Store, gd: &Path) -> Result<Files> {
    let mut out = Files::new();
    for d in INSTANCE_DIRS {
        let p = gd.join(d);
        if p.is_dir() {
            for (rel, e) in store.scan(&p)? {
                out.insert(format!("{d}/{rel}"), e);
            }
        }
    }
    for f in INSTANCE_FILES {
        let p = gd.join(f);
        if p.is_file() {
            out.insert(f.to_string(), store.hash(&p)?);
        }
    }
    Ok(out)
}

pub fn files_hash(f: &Files) -> String {
    use sha1::{Digest, Sha1};
    Sha1::digest(serde_json::to_vec(f).unwrap_or_default()).iter().map(|b| format!("{b:02x}")).collect()
}

/// 本机的局域网地址（写进配对码）。用「往某个地址发包时系统选哪个源地址」来问路由表：
/// 开着 TUN 加速器时，往公网走的是虚拟网卡，往 192.168.x.x 走的才是真的局域网网卡
pub fn local_ips() -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for probe in ["192.168.1.1", "192.168.0.1", "192.168.31.1", "10.0.0.1", "172.16.0.1", "8.8.8.8"] {
        let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") else { continue };
        if s.connect((probe, 9)).is_err() {
            continue;
        }
        if let Ok(a) = s.local_addr() {
            if let IpAddr::V4(v) = a.ip() {
                let ip = v.to_string();
                if v.is_private() && !out.contains(&ip) {
                    out.push(ip);
                }
            }
        }
    }
    out
}

// ---------- 服务端 ----------

pub struct Ctx {
    pub data_dir: PathBuf,
    pub store: Arc<Store>,
    pub host: Arc<dyn Host>,
    pub devices: Mutex<Devices>,
    /// 配对口令和过期时间
    pub pair: Mutex<Option<([u8; 16], u64)>>,
    /// 最近一次 /files 列出的文件：SHA-1 → 本机路径（/get 从这里读）
    served: Mutex<HashMap<String, PathBuf>>,
    /// 谁在玩哪个世界：实例 key/世界 → (设备名, 时间)
    locks: Mutex<HashMap<String, (String, u64)>>,
}

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

fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
        IpAddr::V6(v) => v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80 || v.to_ipv4_mapped().is_some_and(|m| m.is_private() || m.is_loopback()),
    }
}

impl Server {
    /// 开始听。`bind` 测试时传 127.0.0.1，真程序传 0.0.0.0
    pub async fn start(ctx: Arc<Ctx>, bind: IpAddr) -> Result<Server> {
        let listener = match tokio::net::TcpListener::bind(SocketAddr::new(bind, PORT)).await {
            Ok(l) => l,
            Err(_) => tokio::net::TcpListener::bind(SocketAddr::new(bind, 0)).await.context("开不了局域网端口")?,
        };
        let port = listener.local_addr()?.port();
        let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
        let c2 = ctx.clone();
        tokio::spawn(async move {
            loop {
                let (sock, addr) = tokio::select! {
                    r = listener.accept() => match r { Ok(x) => x, Err(_) => continue },
                    _ = &mut stopped => break,
                };
                if !private(addr.ip()) {
                    continue;
                }
                let ctx = c2.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req| handle(ctx.clone(), req));
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(hyper_util::rt::TokioIo::new(sock), svc).await;
                });
            }
        });
        Ok(Server { port, stop: Some(stop) })
    }
}

impl Ctx {
    pub fn new(data_dir: PathBuf, store: Arc<Store>, host: Arc<dyn Host>) -> Ctx {
        let devices = Devices::load(&data_dir);
        Ctx { data_dir, store, host, devices: Mutex::new(devices), pair: Mutex::new(None), served: Mutex::new(HashMap::new()), locks: Mutex::new(HashMap::new()) }
    }

    /// 生成新的配对口令（旧的作废），10 分钟内有效
    pub fn new_pair_token(&self) -> [u8; 16] {
        let t = crypto::random::<16>();
        *self.pair.lock().unwrap() = Some((t, now_ms() + PAIR_TTL_MS));
        t
    }
}

fn reply(status: StatusCode, body: Vec<u8>) -> Response<Full<Bytes>> {
    Response::builder().status(status).body(Full::new(Bytes::from(body))).unwrap()
}

async fn handle(ctx: Arc<Ctx>, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path().to_string();
    let device = req.headers().get("x-ls-device").and_then(|v| v.to_str().ok()).map(String::from);
    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => return Ok(reply(StatusCode::BAD_REQUEST, vec![])),
    };
    if path == "/pair" {
        return Ok(match pair(&ctx, &body) {
            Ok(r) => reply(StatusCode::OK, r),
            Err(_) => reply(StatusCode::FORBIDDEN, b"pair".to_vec()),
        });
    }
    let key = device.as_deref().and_then(|d| ctx.devices.lock().unwrap().client(d).and_then(|k| k.key().ok()));
    let Some(key) = key else { return Ok(reply(StatusCode::FORBIDDEN, b"unknown device".to_vec())) };
    let Ok(plain) = crypto::open(&key, &path, &body) else { return Ok(reply(StatusCode::FORBIDDEN, b"bad key".to_vec())) };
    let out = match unframe(&plain) {
        Ok((v, bytes)) => {
            let ctx2 = ctx.clone();
            let dev = device.unwrap_or_default();
            let p2 = path.clone();
            match tokio::task::spawn_blocking(move || route(&ctx2, &dev, &p2, v, bytes)).await {
                Ok(Ok((v, b))) => frame(&v, &b),
                Ok(Err(e)) => frame(&json!({ "error": format!("{e:#}") }), &[]),
                Err(e) => frame(&json!({ "error": e.to_string() }), &[]),
            }
        }
        Err(e) => frame(&json!({ "error": format!("{e:#}") }), &[]),
    };
    Ok(reply(StatusCode::OK, crypto::seal(&key, &path, &out)))
}

/// 配对：对方用口令派生的密钥加密自己的 id 和名字；我们回一把新密钥
fn pair(ctx: &Ctx, body: &[u8]) -> Result<Vec<u8>> {
    let (token, exp) = ctx.pair.lock().unwrap().context("没有在配对")?;
    if now_ms() > exp {
        bail!("配对码过期了");
    }
    let k = crypto::derive("leafstarter-pair", &token);
    let (v, _) = unframe(&crypto::open(&k, "/pair", body)?)?;
    let id = v["id"].as_str().context("缺 id")?.to_string();
    let name = v["name"].as_str().unwrap_or("另一台设备").chars().take(40).collect::<String>();
    let key = crypto::random::<32>();
    let me = {
        let mut d = ctx.devices.lock().unwrap();
        d.clients.retain(|c| c.id != id);
        d.clients.push(Devices::new_known(&id, &name, &key, vec![], 0)?);
        d.save(&ctx.data_dir)?;
        d.me.clone()
    };
    // 一个口令只能配一台
    *ctx.pair.lock().unwrap() = None;
    ctx.host.changed("devices");
    Ok(crypto::seal(&k, "/pair", &frame(&json!({ "id": me.id, "name": me.name, "key": crypto::b64(&key) }), &[])))
}

fn route(ctx: &Ctx, device: &str, path: &str, v: Value, bytes: Vec<u8>) -> Result<(Value, Vec<u8>)> {
    let key_of = |v: &Value| v["key"].as_str().map(String::from).context("缺实例 key");
    let gd_of = |k: &str| ctx.host.game_dir(k).context("这边找不到这个实例了");
    match path {
        "/info" => {
            let me = ctx.devices.lock().unwrap().me.clone();
            Ok((json!({ "id": me.id, "name": me.name, "instances": ctx.host.instances() }), vec![]))
        }
        "/files" => {
            let gd = gd_of(&key_of(&v)?)?;
            let (dir, files) = match v["world"].as_str() {
                Some(w) => {
                    crate::instance::check_name(w)?;
                    let d = gd.join("saves").join(w);
                    let f = if d.is_dir() { ctx.store.scan(&d)? } else { Files::new() };
                    (d, f)
                }
                None => (gd.clone(), instance_files(&ctx.store, &gd)?),
            };
            {
                let mut s = ctx.served.lock().unwrap();
                for (rel, e) in &files {
                    s.insert(e.sha1.clone(), dir.join(rel));
                }
            }
            Ok((json!({ "files": files, "hash": files_hash(&files) }), vec![]))
        }
        "/get" => {
            let sha = v["sha1"].as_str().context("缺 sha1")?;
            let p = ctx.served.lock().unwrap().get(sha).cloned().unwrap_or_else(|| ctx.store.object_path(sha));
            let data = std::fs::read(&p).with_context(|| "这边没有这个文件了".to_string())?;
            Ok((json!({}), data))
        }
        "/missing" => {
            let list: Vec<String> = serde_json::from_value(v["sha1s"].clone())?;
            // 仓库里有，或者这边硬盘上本来就有一样的文件（/apply 时会先存一份快照，收进仓库），都不用传
            let served = ctx.served.lock().unwrap();
            let have = |s: &String| ctx.store.has(s) || served.get(s).is_some_and(|p| p.is_file());
            Ok((json!({ "missing": list.into_iter().filter(|s| !have(s)).collect::<Vec<_>>() }), vec![]))
        }
        "/put" => {
            let sha = v["sha1"].as_str().context("缺 sha1")?;
            ctx.store.put_bytes(sha, &bytes)?;
            Ok((json!({}), vec![]))
        }
        "/apply" => {
            let key = key_of(&v)?;
            let w = v["world"].as_str().context("缺世界")?;
            crate::instance::check_name(w)?;
            if ctx.host.running(&key) {
                bail!("这边正开着这个实例在玩，先关掉游戏");
            }
            let gd = gd_of(&key)?;
            let dir = gd.join("saves").join(w);
            let files: Files = serde_json::from_value(v["files"].clone())?;
            let base = v["base"].as_str().unwrap_or("");
            let now = if dir.is_dir() { ctx.store.scan(&dir)? } else { Files::new() };
            let now_hash = files_hash(&now);
            if !now.is_empty() && now_hash != base && now != files && !v["force"].as_bool().unwrap_or(false) {
                return Ok((json!({ "conflict": true, "theirs": now_hash }), vec![]));
            }
            let name = ctx.devices.lock().unwrap().client(device).map(|c| c.name.clone()).unwrap_or_default();
            let hkey = crate::commands::sync::world_key(&gd, w);
            if dir.is_dir() {
                ctx.store.snapshot(&hkey, &dir, "这台电脑", "before-sync", crate::commands::sync::KEEP)?;
            }
            // 别处本来就有、没传过来的文件，先收进仓库
            {
                let served = ctx.served.lock().unwrap().clone();
                for e in files.values().filter(|e| !ctx.store.has(&e.sha1)) {
                    let p = served.get(&e.sha1).context("缺文件，再同步一次")?;
                    ctx.store.put_file(p, e)?;
                }
            }
            ctx.store.write_files(&dir, &files)?;
            ctx.store.snapshot(&hkey, &dir, &name, "auto", crate::commands::sync::KEEP)?;
            ctx.host.changed("worlds");
            Ok((json!({ "ok": true, "hash": files_hash(&files) }), vec![]))
        }
        "/lock" => {
            let k = format!("{}/{}", key_of(&v)?, v["world"].as_str().unwrap_or(""));
            let name = ctx.devices.lock().unwrap().client(device).map(|c| c.name.clone()).unwrap_or_default();
            let mut l = ctx.locks.lock().unwrap();
            if v["on"].as_bool().unwrap_or(false) {
                if let Some((who, at)) = l.get(&k) {
                    if *who != name && !v["force"].as_bool().unwrap_or(false) {
                        return Ok((json!({ "ok": false, "holder": who, "since": at }), vec![]));
                    }
                }
                l.insert(k, (name, now_ms()));
            } else {
                l.remove(&k);
            }
            Ok((json!({ "ok": true }), vec![]))
        }
        _ => bail!("不认识的请求 {path}"),
    }
}
