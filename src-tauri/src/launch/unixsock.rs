//! Java 17+ 在 Windows 上建内部管道（Selector、Pipe）用的是 AF_UNIX 套接字文件，放在临时目录里。
//! 有些电脑上（装了某些加速器、安全软件）用户目录下建不了这种文件，Java 就报
//! 「Unable to establish loopback connection」：联机、进服务器、Forge 查更新都会坏。
//!
//! 启动前先用 Windows 自己的接口在几个候选目录试一下（一次运行只试一次）：
//! 默认的临时目录能用就什么都不动；不能用就挑一个能用的，通过 `-Djdk.net.unixdomain.tmpdir` 告诉 Java。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// None：默认临时目录就能用；Some：要指给 Java 的目录
static PICKED: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn java_arg(data_dir: &Path) -> Option<String> {
    PICKED
        .get_or_init(|| pick(data_dir))
        .as_ref()
        .map(|d| format!("-Djdk.net.unixdomain.tmpdir={}", d.display()))
}

#[cfg(windows)]
fn pick(data_dir: &Path) -> Option<PathBuf> {
    let temp = std::env::temp_dir();
    if works(&temp) {
        return None;
    }
    let mut candidates = vec![];
    if let Ok(public) = std::env::var("PUBLIC") {
        candidates.push(PathBuf::from(public).join("LeafStarter").join("sock"));
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        candidates.push(PathBuf::from(pd).join("LeafStarter").join("sock"));
    }
    candidates.push(data_dir.join("sock"));
    candidates.into_iter().find(|d| std::fs::create_dir_all(d).is_ok() && works(d))
}

#[cfg(not(windows))]
fn pick(_data_dir: &Path) -> Option<PathBuf> {
    None
}

/// 在这个目录里建一个 AF_UNIX 监听，再连一次，看行不行
#[cfg(windows)]
pub fn works(dir: &Path) -> bool {
    #[repr(C)]
    struct SockaddrUn {
        family: u16,
        path: [u8; 108],
    }
    #[link(name = "ws2_32")]
    extern "system" {
        fn WSAStartup(version: u16, data: *mut u8) -> i32;
        fn socket(af: i32, kind: i32, proto: i32) -> usize;
        fn bind(s: usize, addr: *const SockaddrUn, len: i32) -> i32;
        fn listen(s: usize, backlog: i32) -> i32;
        fn connect(s: usize, addr: *const SockaddrUn, len: i32) -> i32;
        fn closesocket(s: usize) -> i32;
    }
    const AF_UNIX: i32 = 1;
    const SOCK_STREAM: i32 = 1;
    const INVALID: usize = usize::MAX;

    let file = dir.join(format!("ls-{}-{}.sock", std::process::id(), crate::base::util::now_ms() % 100_000));
    let _ = std::fs::remove_file(&file);
    let path = file.to_string_lossy().into_owned();
    if path.len() >= 108 {
        return false;
    }
    let mut addr = SockaddrUn { family: AF_UNIX as u16, path: [0; 108] };
    addr.path[..path.len()].copy_from_slice(path.as_bytes());
    let len = std::mem::size_of::<SockaddrUn>() as i32;
    let mut wsa = [0u8; 512];
    // SAFETY: 标准的 Winsock 调用；缓冲区够大，套接字用完都关掉
    let ok = unsafe {
        WSAStartup(0x0202, wsa.as_mut_ptr());
        let server = socket(AF_UNIX, SOCK_STREAM, 0);
        if server == INVALID {
            return false;
        }
        let mut ok = bind(server, &addr, len) == 0 && listen(server, 1) == 0;
        if ok {
            let client = socket(AF_UNIX, SOCK_STREAM, 0);
            ok = client != INVALID && connect(client, &addr, len) == 0;
            if client != INVALID {
                closesocket(client);
            }
        }
        closesocket(server);
        ok
    };
    let _ = std::fs::remove_file(&file);
    ok
}

#[cfg(not(windows))]
pub fn works(_dir: &Path) -> bool {
    true
}
