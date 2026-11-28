//! P2P 联机（ROADMAP 5.12 ①）：EasyTier（开源，LGPL-3.0）作为独立程序运行，不链接进我们的程序。
//!
//! - 不装虚拟网卡（`--no-tun`），不要管理员权限，也不开任何监听端口（`--no-listener`），所以不会弹防火墙；
//!   两边都只往外连，靠节点交换地址后打洞，打不通就经节点中转
//! - 主人：虚拟地址固定 10.126.126.1，只放行游戏的局域网端口（`--tcp-whitelist`），朋友碰不到主人电脑上别的服务
//! - 朋友：随机取一个虚拟地址（10.126.126.10～249），把本机一个空闲端口转到 10.126.126.1:<局域网端口>，游戏就连 127.0.0.1:<那个端口>
//! - 房间名和密钥随机生成，放在邀请码里；有邀请码的人才能进
//! - 程序按需下载（先官网 R2，再 GitHub），按 SHA-256 校验；放在 `<数据目录>/easytier/<版本>/`
//! - 启动器退出时，EasyTier 跟着退出（作业对象，关句柄就结束）

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::base::net::{FileJob, Net};

pub const VERSION: &str = "2.6.4";
const ZIP_SHA256: &str = "27af91e270e554709b048bd32327fefd2dfce5062ae1e8701af7550c6f525f84";
const ZIP_NAME: &str = "easytier-windows-x86_64-v2.6.4.zip";
/// 主人的虚拟地址
pub const HOST_IP: &str = "10.126.126.1";
/// 默认的节点：EasyTier 官方的公共节点。用户可以在设置里换成自己的（国内有时连不上官方节点）
pub const DEFAULT_NODES: &[&str] = &["tcp://public.easytier.top:11010"];

/// 邀请码里的房间信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    /// 房间名
    pub n: String,
    /// 密钥
    pub s: String,
    /// 主人的局域网端口
    pub p: u16,
    /// 节点
    pub nodes: Vec<String>,
}

pub fn new_room(port: u16, nodes: &[String]) -> Room {
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    Room { n: format!("ls-{}", hex(&crate::sync::crypto::random::<8>())), s: hex(&crate::sync::crypto::random::<16>()), p: port, nodes: nodes.to_vec() }
}

/// 节点地址只收 tcp / udp / ws / wss，主机名和端口要像样
pub fn clean_nodes(list: &[String]) -> Vec<String> {
    list.iter()
        .map(|x| x.trim().to_string())
        .filter(|x| ["tcp://", "udp://", "ws://", "wss://"].iter().any(|s| x.starts_with(s)) && x.len() < 200 && x.rsplit(':').next().is_some_and(|p| p.trim_end_matches('/').parse::<u16>().is_ok()))
        .take(8)
        .collect()
}

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("easytier").join(VERSION)
}

pub fn exe(data_dir: &Path) -> PathBuf {
    dir(data_dir).join("easytier-core.exe")
}

/// 要从包里解出来的：程序本身和它启动时要加载的 Packet.dll（不装虚拟网卡用不上 wintun.dll）
const FILES: &[&str] = &["easytier-core.exe", "Packet.dll"];

/// 没有就下：先官网，再 GitHub；校验整包的 SHA-256，只解出要用的两个文件和许可说明
pub async fn ensure(net: &Arc<Net>, data_dir: &Path, prog: &Arc<crate::base::net::Progress>) -> Result<PathBuf> {
    let exe = exe(data_dir);
    if FILES.iter().all(|f| dir(data_dir).join(f).is_file()) {
        return Ok(exe);
    }
    if !cfg!(windows) {
        bail!("P2P 联机现在只有 Windows 版");
    }
    let d = dir(data_dir);
    std::fs::create_dir_all(&d)?;
    let zip = d.join(ZIP_NAME);
    let urls = [format!("https://leafstarter.pingyi.me/tools/{ZIP_NAME}"), format!("https://github.com/EasyTier/EasyTier/releases/download/v{VERSION}/{ZIP_NAME}")];
    let mut last = None;
    for u in &urls {
        let _ = std::fs::remove_file(&zip);
        match crate::game::install::download_all(net, &[FileJob { url: u.clone(), path: zip.clone(), sha1: None, size: None }], prog).await {
            Ok(()) => {
                if sha256_file(&zip)? == ZIP_SHA256 {
                    last = None;
                    break;
                }
                last = Some(anyhow::anyhow!("下下来的 EasyTier 校验不对"));
            }
            Err(e) => last = Some(e),
        }
    }
    if let Some(e) = last {
        let _ = std::fs::remove_file(&zip);
        return Err(e.context("EasyTier 没下下来"));
    }
    let mut z = zip::ZipArchive::new(std::io::BufReader::new(std::fs::File::open(&zip)?))?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let name = e.name().rsplit('/').next().unwrap_or("").to_string();
        if FILES.contains(&name.as_str()) {
            let tmp = d.join(format!("{name}.part"));
            std::io::copy(&mut e, &mut std::fs::File::create(&tmp)?)?;
            let _ = std::fs::remove_file(d.join(&name));
            crate::base::util::move_file(&tmp, &d.join(&name))?;
        }
    }
    let _ = std::fs::remove_file(&zip);
    std::fs::write(
        d.join("README.txt"),
        "EasyTier 是开源软件（LGPL-3.0），源码：https://github.com/EasyTier/EasyTier\nLeafStarter 只把它当独立程序运行，没有改动。\n",
    )?;
    if let Some(f) = FILES.iter().find(|f| !d.join(f).is_file()) {
        bail!("EasyTier 包里没有 {f}");
    }
    Ok(exe)
}

fn sha256_file(p: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    std::io::copy(&mut std::fs::File::open(p)?, &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}

// ---------- 进程 ----------

struct Node {
    child: std::process::Child,
    #[cfg(windows)]
    _job: Option<KillJob>,
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// 开着的节点：主人按实例 key，朋友按房间名
static NODES: Mutex<Option<HashMap<String, Node>>> = Mutex::new(None);

fn spawn(exe: &Path, args: &[String], log: &Path) -> Result<Node> {
    let out = std::fs::File::create(log)?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args).stdout(out.try_clone()?).stderr(out).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // 不弹控制台
    }
    let child = cmd.spawn().context("EasyTier 起不来")?;
    #[cfg(windows)]
    let job = {
        use std::os::windows::io::AsRawHandle;
        KillJob::adopt(child.as_raw_handle())
    };
    Ok(Node {
        child,
        #[cfg(windows)]
        _job: job,
    })
}

fn put(key: String, n: Node) {
    NODES.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, n);
}

pub fn stop(key: &str) -> bool {
    NODES.lock().unwrap().as_mut().and_then(|m| m.remove(key)).is_some()
}

pub fn running(key: &str) -> bool {
    let mut g = NODES.lock().unwrap();
    let Some(m) = g.as_mut() else { return false };
    let alive = m.get_mut(key).is_some_and(|n| matches!(n.child.try_wait(), Ok(None)));
    if !alive {
        m.remove(key);
    }
    alive
}

fn base_args(room: &Room) -> Vec<String> {
    // --bind-device false：不把连接绑到物理网卡。开着加速器（TUN 模式）的电脑上，域名解析出来是加速器的假地址，
    // 绑了物理网卡反而连不上节点；我们不开虚拟网卡、不加路由，也就没有绑网卡要防的路由环路
    let mut a: Vec<String> = ["--no-tun", "--no-listener", "--bind-device", "false", "--network-name", &room.n, "--network-secret", &room.s, "--multi-thread", "false"].iter().map(|s| s.to_string()).collect();
    for p in &room.nodes {
        a.push("-p".into());
        a.push(p.clone());
    }
    a
}

/// 主人开房间：虚拟地址 10.126.126.1，只放行游戏端口
pub fn host(exe: &Path, data_dir: &Path, key: &str, room: &Room) -> Result<()> {
    stop(key);
    let mut a = base_args(room);
    a.extend(["-i", HOST_IP, "--hostname", "LeafStarter-host", "--tcp-whitelist", &room.p.to_string(), "--udp-whitelist", &room.p.to_string()].iter().map(|s| s.to_string()));
    let log = data_dir.join("easytier").join("host.log");
    put(format!("host:{key}"), spawn(exe, &a, &log)?);
    Ok(())
}

/// 主人这边等连上节点（日志里出现「new peer added」）；连不上节点的话，邀请码里的 P2P 也就没用
pub async fn wait_node(data_dir: &Path, secs: u64) -> bool {
    let log = data_dir.join("easytier").join("host.log");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < until {
        if std::fs::read_to_string(&log).is_ok_and(|s| s.contains("new peer added")) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    false
}

/// 朋友进房间：本机一个空闲端口转到主人的游戏端口，返回这个端口
pub fn join(exe: &Path, data_dir: &Path, room: &Room) -> Result<u16> {
    let key = format!("join:{}", room.n);
    stop(&key);
    let port = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    let mut a = base_args(room);
    // 直接随机取一个虚拟地址：用 DHCP 要等它分，实测要三四十秒
    let me = format!("10.126.126.{}", 10 + crate::sync::crypto::random::<1>()[0] % 240);
    a.extend(["-i", &me, "--hostname", "LeafStarter-friend", "--port-forward"].iter().map(|s| s.to_string()));
    a.push(format!("tcp://127.0.0.1:{port}/{HOST_IP}:{}", room.p));
    let log = data_dir.join("easytier").join("join.log");
    put(key, spawn(exe, &a, &log)?);
    Ok(port)
}

/// 等转发端口真的能连到主人的游戏（能连上、对 Minecraft 的握手有回应）
pub async fn wait_ready(port: u16, secs: u64) -> bool {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while std::time::Instant::now() < until {
        if mc_ping(port).await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }
    false
}

/// 发一个 Minecraft 服务器状态请求，有回应就算通了（只连上不算：转发端口本地总能连上）
async fn mc_ping(port: u16) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let go = async {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.ok()?;
        // 握手包：包 id 0、协议版本 -1、地址、端口、下一步 1（状态）；再一个状态请求
        let host = b"127.0.0.1";
        let mut body = vec![0x00, 0xff, 0xff, 0xff, 0xff, 0x0f, host.len() as u8];
        body.extend_from_slice(host);
        body.extend_from_slice(&port.to_be_bytes());
        body.push(1);
        let mut pkt = vec![body.len() as u8];
        pkt.extend(body);
        pkt.extend([1, 0]);
        s.write_all(&pkt).await.ok()?;
        let mut b = [0u8; 8];
        let n = s.read(&mut b).await.ok()?;
        (n > 0).then_some(())
    };
    matches!(tokio::time::timeout(std::time::Duration::from_secs(3), go).await, Ok(Some(())))
}

// ---------- 作业对象：启动器退出（句柄关掉）时 EasyTier 跟着结束 ----------
#[cfg(windows)]
struct KillJob(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for KillJob {}

#[cfg(windows)]
impl KillJob {
    fn adopt(process: std::os::windows::io::RawHandle) -> Option<KillJob> {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::*;
        unsafe {
            let job = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32).is_ok()
                && AssignProcessToJobObject(job, HANDLE(process)).is_ok();
            if !ok {
                let _ = windows::Win32::Foundation::CloseHandle(job);
                return None;
            }
            Some(KillJob(job))
        }
    }
}

#[cfg(windows)]
impl Drop for KillJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_and_room() {
        let n = clean_nodes(&["tcp://a.example.com:11010".into(), "http://x:1".into(), "udp://1.2.3.4:abc".into(), " wss://b.example.com:443 ".into()]);
        assert_eq!(n, vec!["tcp://a.example.com:11010".to_string(), "wss://b.example.com:443".to_string()]);
        let r = new_room(25565, &n);
        assert!(r.n.starts_with("ls-") && r.n.len() == 19 && r.s.len() == 32 && r.p == 25565);
    }
}
