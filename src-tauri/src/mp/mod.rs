//! 临时联机（M5，ROADMAP 5.12）：本机能给朋友的地址、邀请码、从游戏日志里认出「对局域网开放」的端口。
//! - 地址：局域网 IPv4（同一个 Wi-Fi）、公网 IPv6（两边都有就能直连）、Radmin VPN（26.x.x.x，只识别不打包）
//! - 邀请码：`LS1.` + base64url(deflate(JSON))，带连接地址和实例清单（MC 版本、加载器、Modrinth 上的 Mod 版本）；
//!   只记来源不传文件，朋友那边从 Modrinth 下
//! - P2P（EasyTier）和樱花穿透要另外的程序和账号，还没接

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, UdpSocket};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// 往一个外面的地址「连」一下（UDP 不真的发包），看系统选了哪个本机地址
fn route_to(target: &str) -> Option<IpAddr> {
    let bind = if target.starts_with('[') { "[::]:0" } else { "0.0.0.0:0" };
    let s = UdpSocket::bind(bind).ok()?;
    s.connect(target).ok()?;
    s.local_addr().ok().map(|a| a.ip())
}

/// 公网 IPv6：2000::/3 里的（排除 ULA、链路本地、文档地址、6to4、Teredo）
pub fn global_v6(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V6(v) => {
            let s = v.segments();
            (s[0] & 0xe000) == 0x2000 && s[0] != 0x2001 || (s[0] == 0x2001 && s[1] != 0 && s[1] != 0x0db8)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Addr {
    /// lan / ipv6 / radmin / server
    pub k: String,
    pub h: String,
    pub p: u16,
}

impl Addr {
    pub fn text(&self) -> String {
        if self.h.contains(':') {
            format!("[{}]:{}", self.h, self.p)
        } else {
            format!("{}:{}", self.h, self.p)
        }
    }
}

/// 网卡上的地址：(网卡名字, 地址)。只看连着的、不是回环的
#[cfg(windows)]
pub fn adapters() -> Vec<(String, IpAddr)> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::*;
    let mut out = vec![];
    let mut size: u32 = 32 * 1024;
    let mut buf = vec![0u64; size as usize / 8 + 1];
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    unsafe {
        let mut r = GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size);
        if r == 111 {
            // ERROR_BUFFER_OVERFLOW：按要的大小再来一次
            buf = vec![0u64; size as usize / 8 + 1];
            r = GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size);
        }
        if r != 0 {
            return out;
        }
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = &*p;
            if a.OperStatus == IfOperStatusUp && a.IfType != 24 {
                let name = format!("{} {}", a.FriendlyName.to_string().unwrap_or_default(), a.Description.to_string().unwrap_or_default());
                let mut u = a.FirstUnicastAddress;
                while !u.is_null() {
                    let sa = (*u).Address.lpSockaddr;
                    if !sa.is_null() {
                        if (*sa).sa_family == AF_INET {
                            let s4 = &*(sa as *const SOCKADDR_IN);
                            out.push((name.clone(), IpAddr::V4(std::net::Ipv4Addr::from(u32::from_be(s4.sin_addr.S_un.S_addr)))));
                        } else if (*sa).sa_family == AF_INET6 {
                            let s6 = &*(sa as *const SOCKADDR_IN6);
                            out.push((name.clone(), IpAddr::V6(std::net::Ipv6Addr::from(s6.sin6_addr.u.Byte))));
                        }
                    }
                    u = (*u).Next;
                }
            }
            p = a.Next;
        }
    }
    out
}

#[cfg(not(windows))]
pub fn adapters() -> Vec<(String, IpAddr)> {
    vec![]
}

/// 虚拟网卡（加速器、虚拟机、VPN）上的局域网地址朋友连不上，不给
fn virtual_nic(name: &str) -> bool {
    let n = name.to_lowercase();
    ["vmware", "virtualbox", "hyper-v", "vethernet", "wintun", "tap-", "tun", "wireguard", "clash", "meta", "vpn", "zerotier", "tailscale", "loopback", "bluetooth", "npcap"].iter().any(|k| n.contains(k))
}

/// 本机能给朋友的地址（端口由游戏决定，这里只给主机）
pub fn hosts() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    let push = |out: &mut Vec<(String, String)>, k: &str, h: String| {
        if !out.iter().any(|(_, x)| *x == h) {
            out.push((k.into(), h));
        }
    };
    for (name, ip) in adapters() {
        match ip {
            IpAddr::V4(v) if v.octets()[0] == 26 && name.to_lowercase().contains("radmin") => push(&mut out, "radmin", v.to_string()),
            IpAddr::V4(v) if v.is_private() && !virtual_nic(&name) => push(&mut out, "lan", v.to_string()),
            IpAddr::V6(_) if global_v6(&ip) && !virtual_nic(&name) => push(&mut out, "ipv6", ip.to_string()),
            _ => {}
        }
    }
    // 网卡列表读不到时的退路：看系统往外走选的地址
    if !out.iter().any(|(k, _)| k == "lan") {
        if let Some(IpAddr::V4(v)) = route_to("223.5.5.5:53") {
            if v.is_private() {
                push(&mut out, "lan", v.to_string());
            }
        }
    }
    if !out.iter().any(|(k, _)| k == "ipv6") {
        if let Some(ip) = route_to("[2400:3200::1]:53").filter(global_v6) {
            push(&mut out, "ipv6", ip.to_string());
        }
    }
    // Radmin VPN 的网段是 26.0.0.0/8：往里面「连」一下，走的是它的网卡就说明装了而且连着
    if !out.iter().any(|(k, _)| k == "radmin") {
        if let Some(IpAddr::V4(v)) = route_to("26.0.0.1:53") {
            if v.octets()[0] == 26 {
                push(&mut out, "radmin", v.to_string());
            }
        }
    }
    // IPv6 一台电脑常有好几个（临时地址），留一个就够
    let mut seen6 = false;
    out.retain(|(k, _)| k != "ipv6" || !std::mem::replace(&mut seen6, true));
    // 朋友那边按顺序用第一个连得上的：公网 IPv6 全球唯一最稳，局域网其次，Radmin 最后
    out.sort_by_key(|(k, _)| match k.as_str() { "ipv6" => 0, "lan" => 1, _ => 2 });
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Invite {
    pub v: u32,
    /// 实例名
    pub name: String,
    pub mc: String,
    pub loader: String,
    pub lv: String,
    /// (Modrinth 项目 id, 版本 id, 文件名)
    pub mods: Vec<(String, String, String)>,
    /// Modrinth 上认不出的 Mod（朋友要自己装）
    pub miss: Vec<String>,
    pub addrs: Vec<Addr>,
    /// 生成时间（毫秒）；超过 24 小时提示可能失效
    pub at: u64,
}

pub fn encode(inv: &Invite) -> Result<String> {
    let json = serde_json::to_vec(inv)?;
    let mut z = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::best());
    z.write_all(&json)?;
    let b = z.finish()?;
    Ok(format!("LS1.{}", b64url(&b)))
}

pub fn decode(code: &str) -> Result<Invite> {
    let code = code.trim().trim_start_matches("leafstarter://join/");
    let body = code.strip_prefix("LS1.").context("这不是 LeafStarter 的邀请码")?;
    let raw = unb64url(body.trim()).context("邀请码不完整（复制的时候少了一段？）")?;
    let mut s = vec![];
    flate2::read::DeflateDecoder::new(&raw[..]).take(1 << 20).read_to_end(&mut s).context("邀请码坏了")?;
    let inv: Invite = serde_json::from_slice(&s).context("邀请码坏了")?;
    if inv.v != 1 {
        bail!("这个邀请码是新版本 LeafStarter 生成的，先更新一下");
    }
    if inv.mods.len() > 1000 || inv.addrs.len() > 10 {
        bail!("邀请码内容不对");
    }
    Ok(inv)
}

fn b64url(b: &[u8]) -> String {
    crate::base::util::base64(b).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

fn unb64url(s: &str) -> Option<Vec<u8>> {
    crate::plugin::sign::unb64(&s.replace('-', "+").replace('_', "/"))
}

/// 点了 `leafstarter://join/<邀请码>` 链接打开的：先记下，界面起来后来取
static LINK: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn link_in(args: &[String]) -> Option<String> {
    args.iter().find(|a| a.to_ascii_lowercase().starts_with("leafstarter://")).map(|a| a.trim().trim_end_matches('/').to_string())
}

pub fn set_link(l: String) {
    *LINK.lock().unwrap() = Some(l);
}

pub fn take_link() -> Option<String> {
    LINK.lock().unwrap().take()
}

/// 游戏日志里「对局域网开放」的端口
pub fn lan_port(line: &str) -> Option<u16> {
    for key in ["Started serving on ", "Local game hosted on port ", "本地游戏已在端口", "Started on "] {
        if let Some(i) = line.find(key) {
            let rest = &line[i + key.len()..];
            let n: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(p) = n.parse::<u16>() {
                if p > 1024 {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// 游戏日志里朋友进出：`[Server thread/INFO]: Steve joined the game` → (Steve, true)
pub fn player_line(line: &str) -> Option<(String, bool)> {
    if !line.contains("Server thread") {
        return None;
    }
    let body = line.rsplit("]: ").next()?.trim();
    for (key, joined) in [(" joined the game", true), (" left the game", false)] {
        if let Some(name) = body.strip_suffix(key) {
            let name = name.trim();
            if !name.is_empty() && name.len() <= 32 && !name.contains(' ') {
                return Some((name.to_string(), joined));
            }
        }
    }
    None
}

/// 朋友那边：哪个地址连得上（TCP 连一下游戏端口，2 秒超时）
pub async fn reachable(a: &Addr) -> bool {
    let addr: Option<SocketAddr> = match a.text().parse() {
        Ok(x) => Some(x),
        Err(_) => tokio::net::lookup_host(a.text()).await.ok().and_then(|mut i| i.next()),
    };
    let Some(addr) = addr else { return false };
    matches!(tokio::time::timeout(std::time::Duration::from_secs(2), tokio::net::TcpStream::connect(addr)).await, Ok(Ok(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_ports() {
        let inv = Invite { v: 1, name: "FO".into(), mc: "1.20.1".into(), loader: "fabric".into(), lv: "0.16.9".into(), mods: vec![("AANobbMI".into(), "abc123".into(), "sodium.jar".into())], miss: vec!["x.jar".into()], addrs: vec![Addr { k: "ipv6".into(), h: "2408:8000::1".into(), p: 51234 }], at: 1 };
        let c = encode(&inv).unwrap();
        assert!(c.starts_with("LS1.") && !c.contains('+') && !c.contains('/'));
        let back = decode(&format!("leafstarter://join/{c}")).unwrap();
        assert_eq!(back.mods, inv.mods);
        assert_eq!(back.addrs[0].text(), "[2408:8000::1]:51234");
        assert!(decode("LS1.abc").is_err());
        assert_eq!(lan_port("[12:00:01] [Server thread/INFO]: Started serving on 51234"), Some(51234));
        assert_eq!(lan_port("[CHAT] Local game hosted on port 2333"), Some(2333));
        assert_eq!(lan_port("[CHAT] 本地游戏已在端口 40000 上开启"), Some(40000));
        assert_eq!(lan_port("Started on 80"), None);
        assert_eq!(player_line("[12:01:02] [Server thread/INFO]: Alex joined the game"), Some(("Alex".into(), true)));
        assert_eq!(player_line("[12:05:00] [Server thread/INFO]: Alex left the game"), Some(("Alex".into(), false)));
        assert_eq!(player_line("[CHAT] Alex joined the game"), None);
        assert!(global_v6(&"2408:8207:1::5".parse().unwrap()));
        assert!(!global_v6(&"fd00::1".parse().unwrap()) && !global_v6(&"fe80::1".parse().unwrap()) && !global_v6(&"2001:db8::1".parse().unwrap()));
    }
}
