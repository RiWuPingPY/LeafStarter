//! Forge / NeoForge 安装器处理：不跑安装器的界面，自己解析 install_profile.json，
//! 依赖库走我们的多源下载，最后用 Java 依次执行安装器里的处理器（和 HMCL / PCL 的做法一样）。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use crate::net::{file_ok, sha1_file, FileJob, Net, Progress};
use crate::util::{join_rel, Coord, OsTarget};

pub struct Processor {
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
    pub outputs: Vec<(String, String)>,
}

pub struct ForgePlan {
    /// 要合并进原版的加载器 JSON
    pub loader_json: Value,
    /// 安装器依赖的库 + 版本 JSON 里的库
    pub jobs: Vec<FileJob>,
    pub processors: Vec<Processor>,
    pub data: HashMap<String, String>,
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let f = std::fs::File::open(path).with_context(|| format!("打开 {}", path.display()))?;
    zip::ZipArchive::new(f).with_context(|| format!("{} 不是有效的 jar（可能下载不完整）", path.display()))
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>> {
    let name = name.trim_start_matches('/');
    let mut entry = zip.by_name(name).with_context(|| format!("安装器里缺少 {}", name))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, dest: &Path) -> Result<()> {
    let data = read_entry(zip, name)?;
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(dest, data)?;
    Ok(())
}

/// 解析安装器，准备好要下载的东西和要执行的处理器。
/// `minecraft_jar` 是原版客户端 jar 的位置，`work_dir` 用来放安装器里解出来的临时数据。
pub fn prepare(installer: &Path, root: &Path, mc: &str, minecraft_jar: &Path, work_dir: &Path, os: OsTarget) -> Result<ForgePlan> {
    let mut zip = open_zip(installer)?;
    let profile: Value = serde_json::from_slice(&read_entry(&mut zip, "install_profile.json")?)
        .context("install_profile.json 格式不对")?;
    let libs_dir = root.join("libraries");

    // 旧版安装器（1.12.2 及以前）：versionInfo 就是版本 JSON，Forge 本体 jar 直接解出来
    if let Some(install) = profile.get("install").filter(|v| v.is_object()) {
        let version_info = profile.get("versionInfo").cloned().ok_or_else(|| anyhow!("旧版安装器缺少 versionInfo"))?;
        let file_path = install.get("filePath").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("缺少 install.filePath"))?;
        let coord = install
            .get("path")
            .and_then(|v| v.as_str())
            .and_then(Coord::parse)
            .ok_or_else(|| anyhow!("缺少 install.path"))?;
        extract_entry(&mut zip, file_path, &join_rel(&libs_dir, &coord.path()))?;
        let jobs = version_info
            .get("libraries")
            .and_then(|l| l.as_array())
            .into_iter()
            .flatten()
            .flat_map(|lib| crate::mojang::library_jobs(lib, &libs_dir, os))
            .collect();
        return Ok(ForgePlan { loader_json: version_info, jobs, processors: vec![], data: HashMap::new() });
    }

    // 新版安装器
    let json_name = profile.get("json").and_then(|v| v.as_str()).unwrap_or("/version.json");
    let loader_json: Value = serde_json::from_slice(&read_entry(&mut zip, json_name)?).context("version.json 格式不对")?;

    // 安装器自带的 maven/ 目录（Forge 本体等）直接解到 libraries
    let names: Vec<String> = zip.file_names().map(String::from).collect();
    for name in names.iter().filter(|n| n.starts_with("maven/") && !n.ends_with('/')) {
        let dest = join_rel(&libs_dir, name.trim_start_matches("maven/"));
        extract_entry(&mut zip, name, &dest)?;
    }

    let mut jobs = vec![];
    for src in [profile.get("libraries"), loader_json.get("libraries")] {
        for lib in src.and_then(|l| l.as_array()).into_iter().flatten() {
            jobs.extend(crate::mojang::library_jobs(lib, &libs_dir, os));
        }
    }

    let mut data: HashMap<String, String> = HashMap::new();
    std::fs::create_dir_all(work_dir)?;
    if let Some(map) = profile.get("data").and_then(|d| d.as_object()) {
        for (key, val) in map {
            let Some(v) = val.get("client").and_then(|c| c.as_str()) else { continue };
            let resolved = if v.starts_with('[') && v.ends_with(']') {
                let c = Coord::parse(v).ok_or_else(|| anyhow!("无法解析 {}", v))?;
                join_rel(&libs_dir, &c.path()).to_string_lossy().into_owned()
            } else if v.starts_with('/') {
                let dest = join_rel(work_dir, v.trim_start_matches('/'));
                extract_entry(&mut zip, v, &dest)?;
                dest.to_string_lossy().into_owned()
            } else {
                v.trim_matches('\'').to_string()
            };
            data.insert(key.clone(), resolved);
        }
    }
    data.insert("SIDE".into(), "client".into());
    data.insert("MINECRAFT_JAR".into(), minecraft_jar.to_string_lossy().into_owned());
    data.insert("MINECRAFT_VERSION".into(), mc.to_string());
    data.insert("ROOT".into(), root.to_string_lossy().into_owned());
    data.insert("INSTALLER".into(), installer.to_string_lossy().into_owned());
    data.insert("LIBRARY_DIR".into(), libs_dir.to_string_lossy().into_owned());

    let mut processors = vec![];
    for p in profile.get("processors").and_then(|p| p.as_array()).into_iter().flatten() {
        if let Some(sides) = p.get("sides").and_then(|s| s.as_array()) {
            if !sides.iter().any(|s| s.as_str() == Some("client")) {
                continue;
            }
        }
        let strs = |key: &str| -> Vec<String> {
            p.get(key).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect()
        };
        let outputs = p
            .get("outputs")
            .and_then(|o| o.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
            .unwrap_or_default();
        processors.push(Processor {
            jar: p.get("jar").and_then(|j| j.as_str()).unwrap_or("").to_string(),
            classpath: strs("classpath"),
            args: strs("args"),
            outputs,
        });
    }
    Ok(ForgePlan { loader_json, jobs: crate::mojang::dedupe(jobs), processors, data })
}

/// 处理器参数里的 `[坐标]` 和 `{变量}` 替换
fn resolve_arg(arg: &str, data: &HashMap<String, String>, libs_dir: &Path) -> String {
    if arg.starts_with('[') && arg.ends_with(']') {
        if let Some(c) = Coord::parse(arg) {
            return join_rel(libs_dir, &c.path()).to_string_lossy().into_owned();
        }
    }
    if arg.starts_with('\'') && arg.ends_with('\'') && arg.len() >= 2 {
        return arg[1..arg.len() - 1].to_string();
    }
    let mut out = String::new();
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        match rest[start..].find('}') {
            Some(end_rel) => {
                let key = &rest[start + 1..start + end_rel];
                match data.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + end_rel + 1]),
                }
                rest = &rest[start + end_rel + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn main_class_of(jar: &Path) -> Result<String> {
    let mut zip = open_zip(jar)?;
    let manifest = String::from_utf8_lossy(&read_entry(&mut zip, "META-INF/MANIFEST.MF")?).into_owned();
    // MANIFEST 每行 72 字节折行，续行以空格开头
    let unfolded = manifest.replace("\r\n ", "").replace("\n ", "");
    unfolded
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:").map(|v| v.trim().to_string()))
        .ok_or_else(|| anyhow!("{} 没有 Main-Class", jar.display()))
}

fn outputs_ok(p: &Processor, data: &HashMap<String, String>, libs_dir: &Path) -> bool {
    if p.outputs.is_empty() {
        return false;
    }
    p.outputs.iter().all(|(k, v)| {
        let path = PathBuf::from(resolve_arg(k, data, libs_dir));
        let expect = resolve_arg(v, data, libs_dir);
        path.is_file() && sha1_file(&path).map(|h| h.eq_ignore_ascii_case(&expect)).unwrap_or(false)
    })
}

/// 依次执行处理器。`on_step(序号, 总数, 说明)` 用来更新任务进度。
pub async fn run_processors(
    plan: &ForgePlan,
    root: &Path,
    java: &Path,
    vanilla: &Value,
    net: &Net,
    prog: &Progress,
    mut on_step: impl FnMut(usize, usize, String),
) -> Result<()> {
    let libs_dir = root.join("libraries");
    let total = plan.processors.len();
    let sep = if cfg!(windows) { ";" } else { ":" };
    for (i, p) in plan.processors.iter().enumerate() {
        prog.check()?;
        let args: Vec<String> = p.args.iter().map(|a| resolve_arg(a, &plan.data, &libs_dir)).collect();
        let short = Coord::parse(&p.jar).map(|c| c.artifact).unwrap_or_else(|| p.jar.clone());
        let task_name = args
            .iter()
            .position(|a| a == "--task")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default();
        on_step(i + 1, total, if task_name.is_empty() { short.clone() } else { format!("{} {}", short, task_name) });

        if outputs_ok(p, &plan.data, &libs_dir) {
            continue;
        }

        // 下载 Mojang 映射表这一步自己来，这样能走镜像
        if task_name == "DOWNLOAD_MOJMAPS" {
            let out = args
                .iter()
                .position(|a| a == "--output")
                .and_then(|i| args.get(i + 1))
                .ok_or_else(|| anyhow!("DOWNLOAD_MOJMAPS 缺少 --output"))?;
            let m = vanilla
                .get("downloads")
                .and_then(|d| d.get("client_mappings"))
                .ok_or_else(|| anyhow!("该版本没有官方映射表"))?;
            let job = FileJob {
                url: m.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
                path: PathBuf::from(out),
                sha1: m.get("sha1").and_then(|s| s.as_str()).map(String::from),
                size: m.get("size").and_then(|s| s.as_u64()),
            };
            if !file_ok(&job.path, job.sha1.as_deref(), job.size).await {
                let tmp = Progress::default();
                net.download(&job, &tmp).await?;
            }
            continue;
        }

        let jar_path = join_rel(&libs_dir, &Coord::parse(&p.jar).ok_or_else(|| anyhow!("处理器坐标无效：{}", p.jar))?.path());
        let main = main_class_of(&jar_path)?;
        let mut cp = vec![jar_path.to_string_lossy().into_owned()];
        for c in &p.classpath {
            if let Some(coord) = Coord::parse(c) {
                cp.push(join_rel(&libs_dir, &coord.path()).to_string_lossy().into_owned());
            }
        }

        let mut cmd = tokio::process::Command::new(java);
        cmd.arg("-cp").arg(cp.join(sep)).arg(&main).args(&args).current_dir(root);
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW，不弹控制台
        let child = cmd.spawn().with_context(|| format!("无法启动 Java：{}", java.display()))?;
        let output = tokio::select! {
            out = child.wait_with_output() => out?,
            _ = async { while !prog.cancelled() { tokio::time::sleep(std::time::Duration::from_millis(300)).await; } } => bail!("已取消"),
        };
        if !output.status.success() {
            let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
            if text.trim().is_empty() {
                text = String::from_utf8_lossy(&output.stdout).into_owned();
            }
            let tail: Vec<&str> = text.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
            bail!("处理器 {} 执行失败（退出码 {:?}）：{}", short, output.status.code(), tail.join(" / "));
        }
        for (k, v) in &p.outputs {
            let path = PathBuf::from(resolve_arg(k, &plan.data, &libs_dir));
            let expect = resolve_arg(v, &plan.data, &libs_dir);
            match sha1_file(&path) {
                Ok(h) if h.eq_ignore_ascii_case(&expect) => {}
                Ok(_) => bail!("处理器 {} 的输出校验不通过：{}", short, path.display()),
                Err(_) => bail!("处理器 {} 没有生成 {}", short, path.display()),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arg_resolution() {
        let mut data = HashMap::new();
        data.insert("SIDE".to_string(), "client".to_string());
        data.insert("MAPPINGS".to_string(), "/x/m.txt".to_string());
        let libs = Path::new("/libs");
        assert_eq!(resolve_arg("{SIDE}", &data, libs), "client");
        assert_eq!(resolve_arg("--side={SIDE}", &data, libs), "--side=client");
        assert_eq!(resolve_arg("'literal'", &data, libs), "literal");
        assert_eq!(resolve_arg("{UNKNOWN}", &data, libs), "{UNKNOWN}");
        let p = resolve_arg("[net.minecraft:client:1.20.1:mappings@txt]", &data, libs);
        assert!(p.replace('\\', "/").ends_with("net/minecraft/client/1.20.1/client-1.20.1-mappings.txt"));
    }
}
