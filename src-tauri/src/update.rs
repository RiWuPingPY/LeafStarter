//! 自动更新（M6）：像 PCL 一样，后台下好、退出时换上，下次打开就是新版本，不弹安装窗口。
//!
//! - 更新信息：GET {base}/api/update/{频道}，base 默认 https://leafstarter.pingyi.me（调试版可以用 LEAFSTARTER_UPDATE_URL 换）
//!   `{ version, date, notes, files: { "windows-x86_64": { url, urls?, size, sha256, sig } } }`
//! - 签名：ed25519，签 `LeafStarter update\n{版本}\n{平台}\n{大小}\n{sha256}`；公钥写死在 UPDATE_KEYS
//!   （调试版可以用 LEAFSTARTER_DEV_UPDATE_KEY 加一把）。签名、大小、哈希有一样不对就不换
//! - 下好的放在 exe 旁边：`<exe>.new`，另记一笔 `<数据目录>/update/pending.json`
//! - 换：正在运行的 exe 能改名、不能覆盖。退出时 exe → `<exe>.old`、`<exe>.new` → exe；
//!   改名不行（杀毒软件拦着之类）就让新 exe 带 `--apply-update` 起来，等旧进程退出后把自己复制过去
//! - 新版本打开后删掉 `.old`；安装版顺手把「应用和功能」里的版本号改掉
//! - 便携版：exe 旁边有 `data` 文件夹，或者 exe 名字里带 portable，数据就放在旁边的 `data/`

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AO};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::plugin::sign;
use crate::state::AppState;

/// 官方更新公钥（base64，指纹 AE2D-F072-E9C4-2565，11-28 生成）。私钥不进仓库，发版时用 LS_UPDATE_KEY 指给发布脚本。
/// 换钥匙时新旧两把并列放一阵，等老版本都更新过来再删旧的
const UPDATE_KEYS: &[&str] = &["rur9kc+BFERCco0k/i+vazmEMDvwiEIpPAdIlkJP+Jk="];
const DEFAULT_BASE: &str = "https://leafstarter.pingyi.me";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn platform() -> &'static str {
    if cfg!(all(windows, target_arch = "x86_64")) {
        "windows-x86_64"
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        "windows-aarch64"
    } else {
        "other"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub url: String,
    /// 备用地址（镜像），按顺序试
    #[serde(default)]
    pub urls: Vec<String>,
    pub size: u64,
    pub sha256: String,
    pub sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub files: HashMap<String, FileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pending {
    version: String,
    sha256: String,
    size: u64,
    #[serde(default)]
    notes: String,
}

/// 界面看的状态
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// idle / checking / latest / downloading / ready / manual / error
    pub phase: String,
    pub latest: Option<String>,
    pub notes: String,
    pub pct: u32,
    pub error: String,
    /// 刚从哪个版本升上来（打开后第一次看状态时有，看过就清）
    pub updated_from: Option<String>,
}

static STATUS: LazyLock<Mutex<Status>> = LazyLock::new(|| Mutex::new(Status { phase: "idle".into(), ..Default::default() }));
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn status() -> Status {
    STATUS.lock().unwrap().clone()
}

pub fn clear_updated() {
    STATUS.lock().unwrap().updated_from = None;
}

fn set(app: &AppHandle, f: impl FnOnce(&mut Status)) {
    let s = {
        let mut g = STATUS.lock().unwrap();
        f(&mut g);
        g.clone()
    };
    let _ = app.emit("update-state", s);
}

pub fn sig_msg(version: &str, platform: &str, size: u64, sha256: &str) -> Vec<u8> {
    format!("LeafStarter update\n{version}\n{platform}\n{size}\n{}", sha256.to_lowercase()).into_bytes()
}

fn keys() -> Vec<Vec<u8>> {
    let mut k: Vec<Vec<u8>> = UPDATE_KEYS.iter().filter_map(|k| sign::unb64(k)).collect();
    if cfg!(debug_assertions) {
        if let Some(x) = std::env::var("LEAFSTARTER_DEV_UPDATE_KEY").ok().and_then(|k| sign::unb64(&k)) {
            k.push(x);
        }
    }
    k
}

fn verify(m: &Manifest, f: &FileInfo) -> bool {
    let sig = match sign::unb64(&f.sig) {
        Some(s) => s,
        None => return false,
    };
    let msg = sig_msg(&m.version, platform(), f.size, &f.sha256);
    keys().iter().any(|k| sign::verify(k, &msg, &sig))
}

/// 版本比大小：1.2.10 > 1.2.9；带 -beta.1 这种后缀的比不带的小
pub fn cmp_version(a: &str, b: &str) -> Ordering {
    fn split(v: &str) -> (Vec<u64>, Option<&str>) {
        let v = v.trim().trim_start_matches('v');
        let (main, pre) = match v.split_once('-') {
            Some((m, p)) => (m, Some(p)),
            None => (v, None),
        };
        (main.split('.').map(|x| x.parse().unwrap_or(0)).collect(), pre)
    }
    let ((ma, pa), (mb, pb)) = (split(a), split(b));
    let n = ma.len().max(mb.len());
    for i in 0..n {
        match ma.get(i).unwrap_or(&0).cmp(mb.get(i).unwrap_or(&0)) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    match (pa, pb) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => {
            let (xs, ys): (Vec<&str>, Vec<&str>) = (x.split('.').collect(), y.split('.').collect());
            for i in 0..xs.len().max(ys.len()) {
                let o = match (xs.get(i), ys.get(i)) {
                    (Some(p), Some(q)) => match (p.parse::<u64>(), q.parse::<u64>()) {
                        (Ok(p), Ok(q)) => p.cmp(&q),
                        _ => p.cmp(q),
                    },
                    (Some(_), None) => Ordering::Greater,
                    (None, Some(_)) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        }
    }
}

fn exe() -> Result<PathBuf> {
    std::env::current_exe().context("找不到程序自己在哪")
}

/// exe 旁边的 `<exe>.<后缀>`
fn side(exe: &Path, ext: &str) -> PathBuf {
    let mut n = exe.file_name().unwrap_or_default().to_os_string();
    n.push(format!(".{ext}"));
    exe.with_file_name(n)
}

fn pending_file(data_dir: &Path) -> PathBuf {
    data_dir.join("update").join("pending.json")
}

fn sha256_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}

/// 便携版的数据目录：exe 旁边有 `data` 文件夹，或者 exe 名字里带 portable
pub fn portable_data() -> Option<PathBuf> {
    let exe = exe().ok()?;
    let dir = exe.parent()?;
    let stem = exe.file_stem()?.to_string_lossy().to_lowercase();
    let data = dir.join("data");
    let marked = data.join(".leafstarter").is_file() || dir.join("portable.txt").is_file() || stem.contains("portable") || stem.contains("便携");
    if marked {
        let _ = std::fs::create_dir_all(&data);
        let _ = std::fs::write(data.join(".leafstarter"), "LeafStarter 便携版的数据，别删\n");
        Some(data)
    } else {
        None
    }
}

/// exe 所在的文件夹能不能写（装在 Program Files 之类的地方就不能自己换）
fn dir_writable(dir: &Path) -> bool {
    let t = dir.join(".leafstarter-write-test");
    let ok = std::fs::write(&t, b"x").is_ok();
    let _ = std::fs::remove_file(&t);
    ok
}

/// 打开时：删掉上次换下来的旧 exe 和没下完的；看是不是刚升级完
pub fn on_start(app: &AppHandle, data_dir: &Path) {
    let Ok(exe) = exe() else { return };
    let _ = std::fs::remove_file(side(&exe, "download"));
    let _ = std::fs::remove_file(side(&exe, "download.part"));
    let pf = pending_file(data_dir);
    let pending: Option<Pending> = std::fs::read(&pf).ok().and_then(|b| serde_json::from_slice(&b).ok());
    match &pending {
        Some(p) if cmp_version(&p.version, VERSION) == Ordering::Greater && side(&exe, "new").is_file() => {
            let mut s = STATUS.lock().unwrap();
            s.phase = "ready".into();
            s.latest = Some(p.version.clone());
            s.notes = p.notes.clone();
        }
        _ => {
            let _ = std::fs::remove_file(&pf);
        }
    }
    let updated = data_dir.join("update").join("updated.json");
    if let Some(v) = std::fs::read(&updated).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        if v["to"].as_str() == Some(VERSION) {
            let mut s = STATUS.lock().unwrap();
            s.updated_from = v["from"].as_str().map(String::from);
            s.notes = v["notes"].as_str().unwrap_or_default().to_string();
        }
        let _ = std::fs::remove_file(&updated);
    }
    // 旧 exe 和换完的 .new（复制方式时新 exe 还在跑一小会儿）：过一会儿再删。
    // 每次删之前都重新看有没有登记：这期间可能刚下好一个新版本
    let pf2 = pf.clone();
    std::thread::spawn(move || {
        for wait in [0u64, 5, 30] {
            std::thread::sleep(Duration::from_secs(wait));
            let _ = std::fs::remove_file(side(&exe, "old"));
            if !pf2.exists() && !BUSY.load(AO::Relaxed) {
                let _ = std::fs::remove_file(side(&exe, "new"));
            }
        }
    });
    #[cfg(windows)]
    if portable_data().is_none() {
        fix_uninstall_version();
    }
    let _ = app;
}

/// 安装版：「应用和功能」里显示的版本号跟着改（安装程序写的是装的时候的版本）
#[cfg(windows)]
fn fix_uninstall_version() {
    use std::os::windows::process::CommandExt;
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\LeafStarter";
    let q = std::process::Command::new("reg").args(["query", key, "/v", "DisplayVersion"]).creation_flags(0x0800_0000).output();
    if let Ok(o) = q {
        let t = String::from_utf8_lossy(&o.stdout);
        if o.status.success() && !t.contains(VERSION) {
            let _ = std::process::Command::new("reg").args(["add", key, "/v", "DisplayVersion", "/t", "REG_SZ", "/d", VERSION, "/f"]).creation_flags(0x0800_0000).output();
        }
    }
}

fn base_url() -> String {
    if cfg!(debug_assertions) {
        if let Ok(u) = std::env::var("LEAFSTARTER_UPDATE_URL") {
            return u.trim_end_matches('/').to_string();
        }
    }
    DEFAULT_BASE.into()
}

/// 看有没有新版本；有就下好放旁边。manual：用户点的（自动更新关着也下）
pub async fn check(app: AppHandle, manual: bool) -> Result<()> {
    if BUSY.swap(true, AO::SeqCst) {
        return Ok(());
    }
    let r = check_inner(&app, manual).await;
    BUSY.store(false, AO::SeqCst);
    if let Err(e) = &r {
        set(&app, |s| {
            s.phase = "error".into();
            s.error = format!("{e:#}");
        });
    }
    r
}

async fn check_inner(app: &AppHandle, manual: bool) -> Result<()> {
    let st = app.state::<AppState>();
    let (off, channel) = {
        let s = st.settings.lock().unwrap();
        (s.update_off, if s.update_channel.is_empty() { "stable".to_string() } else { s.update_channel.clone() })
    };
    if off && !manual {
        return Ok(());
    }
    if status().phase == "ready" {
        return Ok(());
    }
    set(app, |s| {
        s.phase = "checking".into();
        s.error.clear();
    });
    let url = format!("{}/api/update/{channel}?from={VERSION}&p={}", base_url(), platform());
    let host = url.split('/').nth(2).unwrap_or_default().to_string();
    let m: Manifest = st.net.get_json_from(std::slice::from_ref(&url)).await.with_context(|| format!("连不上更新服务器 {host}"))?;
    if cmp_version(&m.version, VERSION) != Ordering::Greater {
        set(app, |s| {
            s.phase = "latest".into();
            s.latest = Some(m.version.clone());
        });
        return Ok(());
    }
    let f = m.files.get(platform()).cloned().with_context(|| format!("{} 还没有这台电脑能用的更新包", m.version))?;
    let exe = exe()?;
    let dir = exe.parent().context("找不到程序所在的文件夹")?.to_path_buf();
    // 这个版本没带公钥，或者装在写不进去的地方：只告诉用户有新版本
    if keys().is_empty() || !dir_writable(&dir) {
        set(app, |s| {
            s.phase = "manual".into();
            s.latest = Some(m.version.clone());
            s.notes = m.notes.clone();
            s.error = if keys().is_empty() { "这个版本不能自动更新，到官网下新版".into() } else { "程序装在了写不进去的位置，到官网下新版".into() };
        });
        return Ok(());
    }
    if !verify(&m, &f) {
        bail!("更新信息的签名不对，没有下载（可能被人改过）");
    }
    set(app, |s| {
        s.phase = "downloading".into();
        s.latest = Some(m.version.clone());
        s.notes = m.notes.clone();
        s.pct = 0;
    });
    // 下到 `<exe>.download`（下的过程中是 `.download.part`），核对过才改名成 `.new`
    let part = side(&exe, "download");
    let prog = Arc::new(crate::base::net::Progress::default());
    let ticker = {
        let (app, prog, size) = (app.clone(), prog.clone(), f.size.max(1));
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let pct = (prog.bytes_done.load(AO::Relaxed) * 100 / size).min(99) as u32;
                set(&app, |s| s.pct = pct);
            }
        })
    };
    let urls: Vec<String> = std::iter::once(f.url.clone()).chain(f.urls.iter().cloned()).collect();
    let job = crate::base::net::FileJob { url: f.url.clone(), path: part.clone(), sha1: None, size: Some(f.size) };
    let r = st.net.download_exact(&urls, &job, &prog).await;
    ticker.abort();
    if let Err(e) = r {
        let _ = std::fs::remove_file(&part);
        return Err(e.context("新版本没下下来"));
    }
    let got = sha256_file(&part)?;
    let size = std::fs::metadata(&part)?.len();
    if !got.eq_ignore_ascii_case(&f.sha256) || size != f.size {
        let _ = std::fs::remove_file(&part);
        bail!("下下来的文件和更新信息对不上，已删掉");
    }
    let new = side(&exe, "new");
    let _ = std::fs::remove_file(&new);
    std::fs::rename(&part, &new).or_else(|_| std::fs::copy(&part, &new).map(|_| ())).context("放不下新版本")?;
    let _ = std::fs::remove_file(&part);
    let pf = pending_file(&st.data_dir);
    std::fs::create_dir_all(pf.parent().unwrap())?;
    std::fs::write(&pf, serde_json::to_vec(&Pending { version: m.version.clone(), sha256: got, size, notes: m.notes.clone() })?)?;
    set(app, |s| {
        s.phase = "ready".into();
        s.pct = 100;
    });
    Ok(())
}

/// 把下好的新版本换上。退出时调（restart = false），或者点「现在重启」（restart = true，换好后带 `--after-update` 起新版本）。
/// 返回换没换
pub fn apply(data_dir: &Path, restart: bool) -> Result<bool> {
    let pf = pending_file(data_dir);
    let Some(p) = std::fs::read(&pf).ok().and_then(|b| serde_json::from_slice::<Pending>(&b).ok()) else { return Ok(false) };
    let exe = exe()?;
    let new = side(&exe, "new");
    if cmp_version(&p.version, VERSION) != Ordering::Greater || !new.is_file() {
        let _ = std::fs::remove_file(&pf);
        return Ok(false);
    }
    // 换之前再核一次：放了这么久，别被换成别的了
    if std::fs::metadata(&new)?.len() != p.size || !sha256_file(&new)?.eq_ignore_ascii_case(&p.sha256) {
        let _ = std::fs::remove_file(&new);
        let _ = std::fs::remove_file(&pf);
        bail!("下好的新版本被改过，已删掉");
    }
    let _ = std::fs::write(data_dir.join("update").join("updated.json"), serde_json::to_vec(&json!({ "from": VERSION, "to": p.version, "notes": p.notes }))?);
    let _ = std::fs::remove_file(&pf);
    let old = side(&exe, "old");
    let _ = std::fs::remove_file(&old);
    let pid = std::process::id().to_string();
    if std::fs::rename(&exe, &old).is_ok() {
        if std::fs::rename(&new, &exe).is_err() && std::fs::copy(&new, &exe).is_err() {
            let _ = std::fs::rename(&old, &exe);
            bail!("换不上新版本");
        }
        if restart {
            spawn(&exe, &["--after-update", &pid])?;
        }
        return Ok(true);
    }
    // 改不了名：让新 exe 自己等这边退出后复制过去
    let target = exe.to_string_lossy().into_owned();
    let mut args = vec!["--apply-update", target.as_str(), pid.as_str()];
    if restart {
        args.push("--restart");
    }
    spawn(&new, &args)?;
    Ok(true)
}

fn spawn(exe: &Path, args: &[&str]) -> Result<()> {
    let mut c = std::process::Command::new(exe);
    c.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // 不跟着这边的控制台走；能脱离作业对象就脱离（不让脱离时退回普通方式）
        c.creation_flags(0x0000_0008 | 0x0100_0000);
        if c.spawn().is_ok() {
            return Ok(());
        }
        c.creation_flags(0x0000_0008);
    }
    c.spawn().context("起不来新版本")?;
    Ok(())
}

/// main 里最先调：`--apply-update <目标> <pid> [--restart]` 是换文件的帮手；`--after-update <pid>` 等旧进程退出再往下走。
/// 返回 Some(退出码) 表示这次只是帮手，做完就退
pub fn pre_start(args: &[String]) -> Option<i32> {
    match args.get(1).map(String::as_str) {
        Some("--after-update") => {
            if let Some(pid) = args.get(2).and_then(|p| p.parse::<u32>().ok()) {
                wait_pid(pid, 20);
            }
            None
        }
        Some("--apply-update") => {
            let (Some(target), Some(pid)) = (args.get(2), args.get(3).and_then(|p| p.parse::<u32>().ok())) else { return Some(2) };
            wait_pid(pid, 30);
            let me = std::env::current_exe().ok()?;
            let target = PathBuf::from(target);
            // 旧进程刚退出时文件可能还被占着一会儿：多试几次
            for _ in 0..40 {
                if std::fs::copy(&me, &target).is_ok() {
                    if args.iter().any(|a| a == "--restart") {
                        let _ = spawn(&target, &[]);
                    }
                    return Some(0);
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Some(1)
        }
        _ => None,
    }
}

/// 等一个进程退出（最多 secs 秒）
fn wait_pid(pid: u32, secs: u32) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, secs * 1000);
            let _ = CloseHandle(h);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, secs);
    }
}

/// 开机后 20 秒看一次，之后每 6 小时看一次
pub fn schedule(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            let _ = check(app.clone(), false).await;
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

/// 发布用：给一个 exe 出更新信息里的那一项
pub fn sign_file(key: &[u8], file: &Path, version: &str, platform: &str, url: &str) -> Result<FileInfo> {
    let size = std::fs::metadata(file)?.len();
    let sha256 = sha256_file(file)?;
    let sig = sign::b64(&sign::sign(key, &sig_msg(version, platform, size, &sha256))?);
    Ok(FileInfo { url: url.to_string(), urls: vec![], size, sha256, sig })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_signatures() {
        assert_eq!(cmp_version("0.2.0", "0.1.9"), Ordering::Greater);
        assert_eq!(cmp_version("0.1.10", "0.1.9"), Ordering::Greater);
        assert_eq!(cmp_version("1.0.0-beta.2", "1.0.0"), Ordering::Less);
        assert_eq!(cmp_version("1.0.0-beta.10", "1.0.0-beta.2"), Ordering::Greater);
        assert_eq!(cmp_version("v1.0", "1.0.0"), Ordering::Equal);
        let k = sign::new_key().unwrap();
        let pk = sign::public_of(&k).unwrap();
        let dir = std::env::temp_dir().join(format!("ls-upd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.exe");
        std::fs::write(&f, b"new exe").unwrap();
        let info = sign_file(&k, &f, "9.0.0", platform(), "https://x/a.exe").unwrap();
        let msg = sig_msg("9.0.0", platform(), info.size, &info.sha256);
        assert!(sign::verify(&pk, &msg, &sign::unb64(&info.sig).unwrap()));
        // 版本号改了签名就不对：不能拿旧包冒充新版本
        assert!(!sign::verify(&pk, &sig_msg("9.0.1", platform(), info.size, &info.sha256), &sign::unb64(&info.sig).unwrap()));
        assert_eq!(side(&f, "new").file_name().unwrap(), "a.exe.new");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
