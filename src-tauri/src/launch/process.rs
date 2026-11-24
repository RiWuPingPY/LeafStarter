//! 拉起游戏进程、读日志、等它退出，退出后找崩溃报告。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::oneshot;

pub struct Exit {
    pub code: Option<i32>,
    /// 是我们按「结束游戏」杀掉的
    pub killed: bool,
}

/// Windows 命令行最长三万多个字符；大整合包的 classpath 会超。Java 9+ 支持 @参数文件
const INLINE_LIMIT: usize = 24_000;

/// @参数文件的写法：每个参数一行，用双引号包起来，反斜杠和引号要转义
fn argfile_text(args: &[String]) -> String {
    args.iter().map(|a| format!("\"{}\"\n", a.replace('\\', "\\\\").replace('"', "\\\""))).collect()
}

pub async fn run(java: &Path, args: &[String], java_major: u32, game_dir: &Path, scratch: &Path, kill: oneshot::Receiver<()>, on_line: Arc<dyn Fn(String) + Send + Sync>) -> Result<Exit> {
    run_tuned(java, args, java_major, game_dir, scratch, kill, on_line, (false, false)).await
}

/// 同 `run`，起来后按实例设置调进程：(优先级高于正常, 后台不降速)
#[allow(clippy::too_many_arguments)]
pub async fn run_tuned(
    java: &Path,
    args: &[String],
    java_major: u32,
    game_dir: &Path,
    scratch: &Path,
    kill: oneshot::Receiver<()>,
    on_line: Arc<dyn Fn(String) + Send + Sync>,
    tune: (bool, bool),
) -> Result<Exit> {
    let mut cmd = tokio::process::Command::new(java);
    let total: usize = args.iter().map(|a| a.len() + 3).sum();
    if total > INLINE_LIMIT && java_major >= 9 {
        std::fs::create_dir_all(scratch)?;
        let file = scratch.join("launch-args.txt");
        std::fs::write(&file, argfile_text(args)).context("写不了启动参数文件")?;
        cmd.arg(format!("@{}", file.display()));
    } else {
        cmd.args(args);
    }
    cmd.current_dir(game_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(false);
    #[cfg(windows)]
    {
        // 不弹黑色的控制台窗口
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().with_context(|| format!("Java 没能启动：{}", java.display()))?;
    #[cfg(windows)]
    if tune.0 || tune.1 {
        if let Some(h) = child.raw_handle() {
            crate::base::gpu::tune(h, tune.0, tune.1);
        }
    }
    #[cfg(not(windows))]
    let _ = tune;
    #[cfg(windows)]
    let job = child.raw_handle().and_then(crate::base::gpu::Job::adopt);
    let out = child.stdout.take().map(|s| tokio::spawn(pump(s, on_line.clone())));
    let err = child.stderr.take().map(|s| tokio::spawn(pump(s, on_line.clone())));

    let mut killed = false;
    let status = tokio::select! {
        s = child.wait() => s?,
        _ = kill => {
            killed = true;
            #[cfg(windows)]
            if let Some(j) = &job {
                j.kill_all();
            }
            let _ = child.kill().await;
            child.wait().await?
        }
    };
    // 把剩下的输出读完
    for h in [out, err].into_iter().flatten() {
        let _ = h.await;
    }
    Ok(Exit { code: status.code(), killed })
}

/// 按行读；不是 UTF-8 的字节（老版本 Java 用系统编码输出）尽量保留下来；log4j 的 XML 转回普通日志行
async fn pump(stream: impl AsyncRead + Unpin, on_line: Arc<dyn Fn(String) + Send + Sync>) {
    let mut fmt = super::logfmt::LogFormatter::new();
    let mut r = BufReader::new(stream);
    let mut buf = Vec::with_capacity(512);
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let raw = String::from_utf8_lossy(&buf);
                for line in fmt.feed(raw.trim_end_matches(['\r', '\n'])) {
                    on_line(line);
                }
            }
        }
    }
}

/// 这次启动之后生成的崩溃报告（crash-reports 里最新的），或者 JVM 自己崩了留下的 hs_err 文件
pub fn find_crash_report(game_dir: &Path, since_ms: u64) -> Option<PathBuf> {
    let newer = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64 + 2000 >= since_ms)
            .unwrap_or(false)
    };
    let pick = |dir: &Path, f: &dyn Fn(&str) -> bool| {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.file_name().map(|n| f(&n.to_string_lossy())).unwrap_or(false) && newer(p))
            .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
    };
    pick(&game_dir.join("crash-reports"), &|n| n.ends_with(".txt")).or_else(|| pick(game_dir, &|n| n.starts_with("hs_err_pid") && n.ends_with(".log")))
}

/// 看到这些日志，说明游戏窗口已经起来了
pub fn is_window_line(line: &str) -> bool {
    const HINTS: [&str; 5] = ["Backend library:", "LWJGL Version", "OpenAL initialized", "Sound engine started", "Created: 1024x"];
    HINTS.iter().any(|h| line.contains(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_argfile() {
        let t = argfile_text(&["-cp".into(), r"C:\a b\x.jar;C:\y.jar".into(), "say \"hi\"".into()]);
        assert_eq!(t, "\"-cp\"\n\"C:\\\\a b\\\\x.jar;C:\\\\y.jar\"\n\"say \\\"hi\\\"\"\n");
    }

    #[test]
    fn spots_window_lines() {
        assert!(is_window_line("[12:00:01] [Render thread/INFO]: Backend library: LWJGL version 3.3.1"));
        assert!(!is_window_line("[12:00:00] [main/INFO]: Setting user: Steve"));
    }
}

/// 验收和二分查冲突用：启动到资源加载完（主菜单前后）再多看几秒，没崩就结束游戏
pub struct Smoke {
    /// 到了主菜单附近，而且之后几秒没崩
    pub ok: bool,
    /// 等太久没到（卡住了）
    pub timed_out: bool,
    pub secs: f64,
    pub code: Option<i32>,
    /// 最后几十行日志（看崩在哪）
    pub tail: Vec<String>,
    pub hit: Option<String>,
}

/// 看到这些日志说明资源加载完了，主菜单马上出来
pub fn reached_menu(line: &str) -> bool {
    line.contains("Sound engine started") || (line.contains("Created: ") && line.contains("atlas")) || line.contains("OpenAL initialized")
}

#[allow(clippy::too_many_arguments)]
pub async fn smoke(java: &Path, args: &[String], java_major: u32, game_dir: &Path, scratch: &Path, limit: std::time::Duration, cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>, on_line: Option<Arc<dyn Fn(String) + Send + Sync>>) -> Result<Smoke> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    let t0 = std::time::Instant::now();
    let (tx, rx) = oneshot::channel::<()>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let kill = {
        let tx = tx.clone();
        move || {
            if let Some(t) = tx.lock().unwrap().take() {
                let _ = t.send(());
            }
        }
    };
    let hit = Arc::new(Mutex::new(None::<String>));
    let timed_out = Arc::new(AtomicBool::new(false));
    let tail = Arc::new(Mutex::new(std::collections::VecDeque::<String>::new()));
    let line_cb: Arc<dyn Fn(String) + Send + Sync> = {
        let (hit, tail, kill) = (hit.clone(), tail.clone(), kill.clone());
        Arc::new(move |l: String| {
            if let Some(f) = &on_line {
                f(l.clone());
            }
            {
                let mut t = tail.lock().unwrap();
                t.push_back(l.clone());
                if t.len() > 60 {
                    t.pop_front();
                }
            }
            let mut h = hit.lock().unwrap();
            if h.is_none() && reached_menu(&l) {
                *h = Some(l);
                // 再等 5 秒，确认不是刚加载完就崩
                let kill = kill.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    kill();
                });
            }
        })
    };
    let watch = {
        let (kill, timed_out) = (kill.clone(), timed_out.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
                    kill();
                    break;
                }
                if t0.elapsed() > limit {
                    timed_out.store(true, Ordering::Relaxed);
                    kill();
                    break;
                }
            }
        })
    };
    let exit = run(java, args, java_major, game_dir, scratch, rx, line_cb).await;
    watch.abort();
    let exit = exit?;
    let hit = hit.lock().unwrap().clone();
    let tail: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
    Ok(Smoke {
        ok: hit.is_some() && exit.killed,
        timed_out: timed_out.load(Ordering::Relaxed) && hit.is_none(),
        secs: t0.elapsed().as_secs_f64(),
        code: exit.code,
        tail,
        hit,
    })
}
