//! 启动核心：版本解析 → 补全文件 → 挑 Java → 解压 natives → 拼参数 → 拉起进程。

pub mod args;
pub mod crash;
pub mod files;
pub mod jvm;
pub mod logfmt;
pub mod natives;
pub mod process;
pub mod resolve;
pub mod unixsock;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::auth::LaunchAuth;
use crate::base::net::{Net, Progress};
use crate::game::{install, java};

/// 实例设置里和启动有关的几项；都不填就是自动
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub memory_mb: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: bool,
    /// 用户自己在实例设置里填的 JVM 参数（只能用户本人填，整合包和同步都带不进来）
    pub jvm_args: String,
    /// 指定 Java 可执行文件；空的就自动挑
    pub java_path: String,
}

pub struct Prepared {
    pub java: PathBuf,
    pub java_major: u32,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
}

/// 进度：(阶段, 百分比, 说明)
pub type Report = Arc<dyn Fn(&str, u32, String) + Send + Sync>;

/// 没写 component 时按大版本号猜官方运行时的名字
fn default_component(major: u32) -> &'static str {
    match major {
        0..=8 => "jre-legacy",
        9..=16 => "java-runtime-alpha",
        17..=20 => "java-runtime-gamma",
        21..=24 => "java-runtime-delta",
        _ => "java-runtime-epsilon",
    }
}

pub async fn prepare(net: &Arc<Net>, data_dir: &Path, root: &Path, name: &str, auth: &LaunchAuth, opts: &Options, report: Report) -> Result<Prepared> {
    let r = resolve::resolve(root, name)?;
    files::check_generated(&r.json, root)?;
    let (game_dir, _) = crate::instance::game_dir(root, name);
    std::fs::create_dir_all(&game_dir)?;

    // 文件：缺的补上，进度按字节算
    report("files", 0, "检查文件".into());
    let prog = Arc::new(Progress::default());
    let jobs = files::required(net, root, &r, &prog).await?;
    let missing = files::missing(jobs).await;
    if !missing.is_empty() {
        let n = missing.len();
        report("files", 1, format!("补上缺的 {} 个文件", n));
        let (p2, rep2) = (prog.clone(), report.clone());
        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(400)).await;
                let total = p2.bytes_total.load(Ordering::Relaxed).max(1);
                let pct = (p2.bytes_done.load(Ordering::Relaxed) * 100 / total).min(99) as u32;
                rep2("files", pct.max(1), format!("补上缺的 {} 个文件", n));
            }
        });
        let res = install::download_all(net, &missing, &prog).await;
        ticker.abort();
        res.context("补文件的时候有文件没下载成功")?;
    }
    report("files", 100, "文件都在".into());
    let game_assets = files::legacy_assets(&r.json, root, &game_dir)?;

    // Java
    let (need, component) = jvm::need_of(&r.json);
    let (java_path, java_major) = if !opts.java_path.trim().is_empty() {
        let p = PathBuf::from(opts.java_path.trim());
        if !p.is_file() {
            bail!("实例设置里指定的 Java 不存在：{}", p.display());
        }
        (p, need)
    } else {
        let list = java::detect(data_dir);
        match jvm::choose(&list, need) {
            Some(j) => (PathBuf::from(&j.path), j.major),
            None => {
                report("java", 0, format!("下载 Java {}", need));
                let comp = component.unwrap_or_else(|| default_component(need).to_string());
                let p = java::download_runtime(net, data_dir, &Arc::new(Progress::default()), Some(&comp)).await.with_context(|| format!("这个版本要 Java {}，本机没有，下载也没成功", need))?;
                (p, need)
            }
        }
    };
    report("java", 100, format!("Java {}", java_major));

    // natives
    let natives_dir = root.join("versions").join(name).join(format!("{}-natives", name));
    natives::extract(&r.json, &root.join("libraries"), &natives_dir)?;

    // 参数
    let mut args = build_args(&r, root, name, &game_dir, &game_assets, &natives_dir, auth, opts, java_major)?;
    if java_major >= 16 {
        if let Some(a) = unixsock::java_arg(data_dir) {
            args.insert(0, a);
        }
    }
    Ok(Prepared { java: java_path, java_major, args, game_dir, natives_dir })
}

#[allow(clippy::too_many_arguments)]
fn build_args(r: &resolve::Resolved, root: &Path, name: &str, game_dir: &Path, game_assets: &Path, natives_dir: &Path, auth: &LaunchAuth, opts: &Options, _java_major: u32) -> Result<Vec<String>> {
    let json = &r.json;
    let main = json.get("mainClass").and_then(|m| m.as_str()).context("版本文件里没有 mainClass，这个版本装得不完整")?;
    let width = opts.width.unwrap_or(854);
    let height = opts.height.unwrap_or(480);
    let custom_res = !opts.fullscreen && (opts.width.is_some() || opts.height.is_some());
    let f = args::Features { custom_resolution: custom_res };
    let libs = root.join("libraries");
    let sep = if cfg!(windows) { ";" } else { ":" };
    let cp = args::classpath(json, &libs, &r.jar).iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(sep);
    let s = |p: &Path| p.to_string_lossy().into_owned();
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", auth.name.clone()),
        ("version_name", name.to_string()),
        ("game_directory", s(game_dir)),
        ("assets_root", s(&root.join("assets"))),
        ("game_assets", s(game_assets)),
        ("assets_index_name", json.get("assetIndex").and_then(|i| i.get("id")).and_then(|i| i.as_str()).unwrap_or("legacy").to_string()),
        ("auth_uuid", auth.uuid.clone()),
        ("auth_access_token", auth.token.clone()),
        ("auth_session", auth.token.clone()),
        ("clientid", String::new()),
        ("auth_xuid", "0".to_string()),
        ("user_type", auth.user_type.to_string()),
        ("user_properties", "{}".to_string()),
        ("version_type", "LeafStarter".to_string()),
        ("natives_directory", s(natives_dir)),
        ("launcher_name", "LeafStarter".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("classpath", cp),
        ("classpath_separator", sep.to_string()),
        ("library_directory", s(&libs)),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
    ]);

    let mem = opts.memory_mb.unwrap_or_else(|| jvm::auto_memory_mb(jvm::total_memory_mb()));
    let mut out: Vec<String> = vec![
        format!("-Xmx{}m", mem),
        "-XX:+UseG1GC".into(),
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:G1NewSizePercent=20".into(),
        "-XX:G1ReservePercent=20".into(),
        "-XX:MaxGCPauseMillis=50".into(),
        "-XX:G1HeapRegionSize=32M".into(),
        // 日志按 UTF-8 输出，启动器这边读得懂中文
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
    ];
    out.extend(opts.jvm_args.split_whitespace().map(String::from));
    match json.get("arguments").and_then(|a| a.get("jvm")) {
        Some(jvm) => out.extend(args::collect(Some(jvm), f).iter().map(|a| args::fill(a, &vars))),
        None => out.extend(
            ["-Djava.library.path=${natives_directory}", "-Dminecraft.launcher.brand=${launcher_name}", "-Dminecraft.launcher.version=${launcher_version}", "-cp", "${classpath}"]
                .iter()
                .map(|a| args::fill(a, &vars)),
        ),
    }
    // 官方的 log4j 配置（顺带堵上老版本的 log4j 漏洞）
    if let Some(client) = json.get("logging").and_then(|l| l.get("client")) {
        let id = client.get("file").and_then(|f| f.get("id")).and_then(|i| i.as_str());
        let arg = client.get("argument").and_then(|a| a.as_str());
        if let (Some(id), Some(arg)) = (id, arg) {
            let p = root.join("assets").join("log_configs").join(id);
            if p.is_file() {
                out.push(arg.replace("${path}", &s(&p)));
            }
        }
    }
    out.push(main.to_string());
    match json.get("arguments").and_then(|a| a.get("game")) {
        Some(game) => out.extend(args::collect(Some(game), f).iter().map(|a| args::fill(a, &vars))),
        None => {
            let legacy = json.get("minecraftArguments").and_then(|m| m.as_str()).unwrap_or("");
            out.extend(legacy.split_whitespace().map(|a| args::fill(a, &vars)));
            if custom_res {
                out.extend(["--width".into(), width.to_string(), "--height".into(), height.to_string()]);
            }
        }
    }
    if opts.fullscreen {
        out.push("--fullscreen".into());
    }
    Ok(out)
}

/// 日志里不能出现令牌：给界面和导出看的日志一律打码
pub fn redact(line: &str, token: &str) -> String {
    if token.len() > 8 && line.contains(token) {
        line.replace(token, "********")
    } else {
        line.to_string()
    }
}
