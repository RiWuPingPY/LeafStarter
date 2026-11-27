//! 启动核心：版本解析 → 补全文件 → 挑 Java → 解压 natives → 拼参数 → 拉起进程。

pub mod args;
pub mod bisect;
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
    /// 性能调度（5.8）：强制用高性能显卡、进程优先级高于正常、切到后台不降速
    pub gpu_high: bool,
    pub priority_high: bool,
    pub no_throttle: bool,
    /// 低配优化包加进来的文件（撤掉时只删这些）
    pub perf_mods: Vec<String>,
    /// 从哪个整合包装的：{ format, name, version, project? }
    pub pack: Option<serde_json::Value>,
    /// 启动后直接进这个服务器（host:port，IPv6 写成 [地址]:端口）；只这一次，不存进实例设置
    #[serde(default, skip_serializing)]
    pub join: Option<String>,
}

pub struct Prepared {
    pub java: PathBuf,
    pub java_major: u32,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    /// (优先级高于正常, 后台不降速)
    pub tune: (bool, bool),
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
        let major = java::detect(data_dir).iter().find(|j| std::path::Path::new(&j.path) == p).map(|j| j.major).unwrap_or(need);
        (p, major)
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
    // 强制用独显：开了就给这个 Java 设「高性能」，关了就撤掉我们设的那条
    if let Err(e) = crate::base::gpu::prefer_high(&java_path, opts.gpu_high) {
        eprintln!("显卡偏好没设上：{e:#}");
    }

    // natives
    let natives_dir = root.join("versions").join(name).join(format!("{}-natives", name));
    natives::extract(&r.json, &root.join("libraries"), &natives_dir)?;

    // 外置登录：authlib-injector 放在所有参数最前面
    let agent = match &auth.ygg {
        Some((api, meta)) => {
            report("java", 100, "准备外置登录".into());
            let jar = crate::auth::ygg::ensure_agent(data_dir).await?;
            vec![format!("-javaagent:{}={}", jar.display(), api), format!("-Dauthlibinjector.yggdrasil.prefetched={}", crate::base::util::base64(meta.as_bytes()))]
        }
        None => vec![],
    };

    // 参数
    if opts.join.as_deref().is_some_and(|j| !j.trim().is_empty()) {
        skip_onboarding(&game_dir);
    }
    let mut args = build_args(&r, root, name, &game_dir, &game_assets, &natives_dir, auth, opts, java_major)?;
    args.splice(0..0, agent);
    if java_major >= 16 {
        if let Some(a) = unixsock::java_arg(data_dir) {
            args.insert(0, a);
        }
    }
    Ok(Prepared { java: java_path, java_major, args, game_dir, natives_dir, tune: (opts.priority_high, opts.no_throttle) })
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
    // 用户在 JVM 参数里自己选了垃圾回收器（-XX:+UseZGC 这类）：不再加 G1 那套，两个一起 JVM 会拒绝启动
    let own_gc = opts.jvm_args.split_whitespace().any(|a| a.starts_with("-XX:+Use") && a.ends_with("GC"));
    let gc: Vec<String> = if own_gc {
        vec![]
    } else {
        ["-XX:+UseG1GC", "-XX:+UnlockExperimentalVMOptions", "-XX:G1NewSizePercent=20", "-XX:G1ReservePercent=20", "-XX:MaxGCPauseMillis=50", "-XX:G1HeapRegionSize=32M"].map(String::from).to_vec()
    };
    let mut out: Vec<String> = vec![format!("-Xmx{}m", mem)];
    out.extend(gc);
    out.extend([
        // 日志按 UTF-8 输出，启动器这边读得懂中文
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
    ]);
    out.extend(opts.jvm_args.split_whitespace().map(String::from));
    // 新写法的 jvm 参数里没有 classpath（老版本，或者继承老版本、只写了几条额外参数的加载器版本）：先补上老写法的那几条
    let jvm = json.get("arguments").and_then(|a| a.get("jvm"));
    if !jvm.is_some_and(|j| j.to_string().contains("${classpath}")) {
        out.extend(
            ["-Djava.library.path=${natives_directory}", "-Dminecraft.launcher.brand=${launcher_name}", "-Dminecraft.launcher.version=${launcher_version}", "-cp", "${classpath}"]
                .iter()
                .map(|a| args::fill(a, &vars)),
        );
    }
    out.extend(args::collect(jvm, f).iter().map(|a| args::fill(a, &vars)));
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
    // 新写法的 game 参数里有用户名就用它；没有（老版本，或者继承老版本的加载器版本只写了几条额外的）就先用老写法那一行
    let game = json.get("arguments").and_then(|a| a.get("game"));
    if !game.is_some_and(|g| g.to_string().contains("${auth_player_name}")) {
        let legacy = json.get("minecraftArguments").and_then(|m| m.as_str()).unwrap_or("");
        out.extend(legacy.split_whitespace().map(|a| args::fill(a, &vars)));
        if custom_res {
            out.extend(["--width".into(), width.to_string(), "--height".into(), height.to_string()]);
        }
    }
    out.extend(args::collect(game, f).iter().map(|a| args::fill(a, &vars)));
    if opts.fullscreen {
        out.push("--fullscreen".into());
    }
    if let Some(j) = opts.join.as_deref().map(str::trim).filter(|j| !j.is_empty()) {
        out.extend(join_args(json, j));
    }
    Ok(out)
}

/// 直接进服务器时，新实例第一次打开会先停在「无障碍设置」引导页，等点了继续才进服务器：把它关掉。
/// options.txt 还没有时只在没有「默认选项」类 Mod 的模板时才新建（有模板的话改模板，不抢它的活）
fn skip_onboarding(game_dir: &Path) {
    let fix = |p: &Path| {
        if let Ok(t) = std::fs::read_to_string(p) {
            if t.contains("onboardAccessibility:true") {
                let _ = std::fs::write(p, t.replace("onboardAccessibility:true", "onboardAccessibility:false"));
            }
        }
    };
    let opt = game_dir.join("options.txt");
    if opt.exists() {
        return fix(&opt);
    }
    let templates = [game_dir.join("config/yosbr/options.txt"), game_dir.join("config/defaultoptions/options.txt"), game_dir.join("defaultconfigs/options.txt")];
    let mut has_template = false;
    for t in templates.iter().filter(|t| t.exists()) {
        has_template = true;
        fix(t);
    }
    if !has_template {
        let _ = std::fs::write(&opt, "onboardAccessibility:false
");
    }
}

/// 直接进服务器：1.20 起用 --quickPlayMultiplayer，之前用 --server / --port
pub fn join_args(json: &serde_json::Value, addr: &str) -> Vec<String> {
    // world:<存档文件夹>：直接进单人世界（1.20 起的 --quickPlaySingleplayer；更老的版本做不到，只是正常启动）
    if let Some(folder) = addr.strip_prefix("world:") {
        let ok = json.get("arguments").and_then(|a| a.get("game")).is_some_and(|g| g.to_string().contains("quickPlaySingleplayer"));
        return if ok && !folder.is_empty() { vec!["--quickPlaySingleplayer".into(), folder.to_string()] } else { vec![] };
    }
    let quick = json.get("arguments").and_then(|a| a.get("game")).is_some_and(|g| g.to_string().contains("quickPlayMultiplayer"));
    if quick {
        return vec!["--quickPlayMultiplayer".into(), addr.to_string()];
    }
    // [ipv6]:port 或 host:port；没写端口就是 25565
    let (host, port) = match addr.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() && (!h.contains(':') || h.starts_with('[')) => (h.trim_start_matches('[').trim_end_matches(']').to_string(), p.to_string()),
        _ => (addr.trim_start_matches('[').trim_end_matches(']').to_string(), "25565".to_string()),
    };
    vec!["--server".into(), host, "--port".into(), port]
}

/// 日志里不能出现令牌：给界面和导出看的日志一律打码
pub fn redact(line: &str, token: &str) -> String {
    if token.len() > 8 && line.contains(token) {
        line.replace(token, "********")
    } else {
        line.to_string()
    }
}
