//! 一次安装的完整流程：原版 → 加载器 → 下载 → （Forge 处理器）→ 写版本 JSON → 同步到启动器目录。

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::game::launchers::Target;
use crate::game::loaders::{self, LoaderKind};
use crate::game::mojang;
use crate::base::net::{FileJob, Net, Progress};
use crate::base::task::TaskEntry;
use crate::base::util::{human_size, sanitize_version_name, OsTarget};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub target_id: String,
    pub mc_version: String,
    pub name: String,
    pub loader: Option<LoaderReq>,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderReq {
    pub kind: String,
    pub id: String,
    pub version: String,
}

pub struct Env {
    /// 命令行模式下没有窗口，为 None；只有安卓写授权目录时用到
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub app: Option<AppHandle>,
    pub net: Arc<Net>,
    pub data_dir: PathBuf,
}

fn is_android() -> bool {
    cfg!(target_os = "android")
}

/// 批量下载，全部完成后再统一报错（不会因为一个文件失败就丢掉其他已下好的）
pub async fn download_all(net: &Arc<Net>, jobs: &[FileJob], prog: &Arc<Progress>) -> Result<()> {
    prog.files_total.fetch_add(jobs.len() as u64, Ordering::Relaxed);
    prog.bytes_total.fetch_add(jobs.iter().filter_map(|j| j.size).sum(), Ordering::Relaxed);
    let threads = net.threads();
    // 每个下载拿自己的 Arc，免得借用的闭包让外层 future 不满足 Send
    let futs = jobs.iter().cloned().map(|job| {
        let net = net.clone();
        let prog = prog.clone();
        async move { net.download(&job, &prog).await }
    });
    let results: Vec<Result<()>> = futures_util::stream::iter(futs).buffer_unordered(threads).collect().await;
    prog.check()?;
    let errors: Vec<String> = results.into_iter().filter_map(|r| r.err()).map(|e| format!("{:#}", e)).collect();
    if !errors.is_empty() {
        bail!("{} 个文件没下载成功。第一个错误：{}", errors.len(), errors[0]);
    }
    Ok(())
}

fn ensure_launcher_profiles(root: &Path) -> Result<()> {
    let p = root.join("launcher_profiles.json");
    if !p.exists() {
        crate::base::util::write_json(&p, &json!({ "profiles": {}, "selectedProfile": null, "version": 3 }))?;
    }
    Ok(())
}

pub async fn run(env: &Env, target: Target, req: InstallRequest, task: Arc<TaskEntry>) -> Result<Option<String>> {
    let net = env.net.clone();
    let prog = task.prog.clone();
    let os = if is_android() { OsTarget::Linux } else { OsTarget::current() };
    let saf = target.access == "saf";
    match target.access.as_str() {
        "direct" | "saf" => {}
        "need-saf" => bail!("还没授权访问「{}」的目录，请先在「启动器」页点授权", target.launcher),
        "need-permission" => bail!("需要先开启「所有文件访问」权限才能写入 {}", target.path),
        other => bail!("目标目录不可用（{}）", other),
    }

    let root: PathBuf = if saf { env.data_dir.join("staging").join(".minecraft") } else { PathBuf::from(&target.path) };
    std::fs::create_dir_all(&root).with_context(|| format!("无法创建 {}", root.display()))?;

    let loader_kind = match &req.loader {
        Some(l) => Some(LoaderKind::parse(&l.kind).ok_or_else(|| anyhow!("不支持的加载器：{}", l.kind))?),
        None => None,
    };
    // 安卓上跑不了 Forge 安装器：先装原版，再把安装器交给启动器
    let android_forge = is_android() && loader_kind.map_or(false, |k| k.needs_installer());

    let mut name = sanitize_version_name(&req.name);
    if android_forge {
        name = req.mc_version.clone();
    }
    if name.is_empty() {
        bail!("版本名不能为空");
    }
    let exists = if saf { target.versions.iter().any(|v| v.name == name) } else { root.join("versions").join(&name).join(format!("{}.json", name)).is_file() };
    if exists && !req.overwrite && !android_forge {
        bail!("「{}」里已经有叫「{}」的版本了，换个名字再装", target.launcher, name);
    }

    // ---------- 原版 ----------
    task.stage("获取版本清单");
    let manifest = mojang::fetch_manifest(&net).await?;
    let mv = manifest
        .versions
        .iter()
        .find(|v| v.id == req.mc_version)
        .ok_or_else(|| anyhow!("版本清单里没有 {}", req.mc_version))?
        .clone();
    task.stage(format!("读取 {} 的版本信息", mv.id));
    let vanilla = mojang::fetch_version_json(&net, &mv).await?;
    let mut jobs = mojang::base_jobs(&vanilla, &root, &name, os);

    let mut asset_index: Option<Value> = None;
    if let Some(idx_job) = mojang::asset_index_job(&vanilla, &root) {
        task.stage("获取资源索引");
        net.download(&idx_job, &Progress::default()).await?;
        let idx = crate::base::util::read_json(&idx_job.path)?;
        jobs.extend(mojang::asset_object_jobs(&idx, &root));
        asset_index = Some(idx);
    }

    // ---------- 加载器 ----------
    let mut loader_json: Option<Value> = None;
    let mut forge_plan = None;
    let mut installer_path: Option<PathBuf> = None;
    if let (Some(kind), Some(l)) = (loader_kind, &req.loader) {
        match kind {
            LoaderKind::Fabric | LoaderKind::Quilt => {
                task.stage(format!("获取 {} {} 的配置", kind.label(), l.version));
                let profile = loaders::fabric_profile(&net, kind, &req.mc_version, &l.id).await?;
                let libs_dir = root.join("libraries");
                for lib in profile.get("libraries").and_then(|x| x.as_array()).into_iter().flatten() {
                    jobs.extend(mojang::library_jobs(lib, &libs_dir, os));
                }
                loader_json = Some(profile);
            }
            LoaderKind::Forge | LoaderKind::NeoForge => {
                task.stage(format!("下载 {} {} 安装器", kind.label(), l.version));
                let file_name = l.id.rsplit('/').next().unwrap_or("installer.jar").to_string();
                let inst = env.data_dir.join("cache").join("installers").join(&file_name);
                let job = FileJob { url: loaders::installer_url(kind, &l.id), path: inst.clone(), sha1: None, size: None };
                net.download(&job, &Progress::default()).await?;
                if !android_forge {
                    task.stage(format!("解析 {} 安装器", kind.label()));
                    let work = env.data_dir.join("cache").join("forge-work").join(crate::game::launchers::target_id(&name));
                    let mc_jar = root.join("versions").join(&name).join(format!("{}.jar", name));
                    let plan = crate::game::forge::prepare(&inst, &root, &req.mc_version, &mc_jar, &work, os)?;
                    jobs.extend(plan.jobs.iter().cloned());
                    loader_json = Some(plan.loader_json.clone());
                    forge_plan = Some(plan);
                }
                installer_path = Some(inst);
            }
        }
    }

    // ---------- 下载 ----------
    let jobs = mojang::dedupe(jobs);
    task.reset_counters();
    let total: u64 = jobs.iter().filter_map(|j| j.size).sum();
    task.stage(format!("下载 {} 个文件（约 {}）", jobs.len(), human_size(total)));
    download_all(&net, &jobs, &prog).await?;

    // ---------- Forge / NeoForge 处理器 ----------
    if let (Some(plan), Some(kind)) = (&forge_plan, loader_kind) {
        if !plan.processors.is_empty() {
            task.stage("查找 Java");
            let found = crate::game::java::detect(&env.data_dir);
            let java = match crate::game::java::pick(&found, 17) {
                Some(j) => {
                    task.log(format!("使用 Java {}（{}）", j.version, j.source));
                    PathBuf::from(&j.path)
                }
                None => {
                    task.stage("本机没有合适的 Java，下载官方 Java 运行时");
                    task.reset_counters();
                    crate::game::java::download_runtime(&net, &env.data_dir, &prog, None).await?
                }
            };
            let label = kind.label();
            crate::game::forge::run_processors(plan, &root, &java, &vanilla, &net, &prog, |i, n, what| {
                task.stage(format!("安装 {}（{}/{}）：{}", label, i, n, what));
            })
            .await?;
        }
    }

    // ---------- 写版本 JSON ----------
    task.stage("写入版本信息");
    let merged = mojang::merge(&vanilla, loader_json.as_ref(), &name, &req.mc_version);
    let json_path = root.join("versions").join(&name).join(format!("{}.json", name));
    crate::base::util::write_json(&json_path, &merged)?;
    ensure_launcher_profiles(&root)?;

    let mut note: Option<String> = None;
    let mut installer_rel: Option<String> = None;
    if android_forge {
        if let (Some(inst), Some(kind)) = (&installer_path, loader_kind) {
            // 安装器同时放进启动器目录的 installers/ 里：启动器的文件选择器一定能看到自己的目录
            let file_name = inst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let rel = format!("installers/{}", file_name);
            let dest = crate::base::util::join_rel(&root, &rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(inst, &dest)?;
            installer_rel = Some(rel.clone());
            let public = save_installer_public(inst);
            let mut places = vec![format!("启动器目录的 .minecraft/{}", rel)];
            if let Some(p) = public {
                places.push(p);
            }
            note = Some(format!(
                "安卓上没法直接运行 {} 安装器。已经装好原版 {}，安装器放在：{}。请在启动器里用「执行 jar」或「安装 {}」选这个文件完成安装。",
                kind.label(),
                req.mc_version,
                places.join("，以及 "),
                kind.label()
            ));
        }
    }

    // ---------- 同步到启动器（安卓授权目录）----------
    if saf {
        let mut files: Vec<String> = vec![
            format!("versions/{0}/{0}.json", name),
            format!("versions/{0}/{0}.jar", name),
        ];
        files.extend(mojang::library_paths(&merged, os).into_iter().map(|p| format!("libraries/{}", p)));
        if let Some(idx) = merged.get("assetIndex").and_then(|i| i.get("id")).and_then(|i| i.as_str()) {
            files.push(format!("assets/indexes/{}.json", idx));
        }
        if let Some(idx) = &asset_index {
            files.extend(mojang::asset_object_paths(idx));
        }
        if let Some(id) = merged.pointer("/logging/client/file/id").and_then(|i| i.as_str()) {
            files.push(format!("assets/log_configs/{}", id));
        }
        files.push("launcher_profiles.json".into());
        files.extend(installer_rel.clone());
        files.retain(|f| crate::base::util::join_rel(&root, f).is_file());
        files.sort();
        files.dedup();
        crate::base::platform::copy_to_saf(env, &target, &root, &files, &task).await?;
    }

    if let Some(n) = note {
        task.note(n);
    }
    task.stage("完成");
    Ok(Some(if saf { target.path.clone() } else { json_path.parent().unwrap().to_string_lossy().into_owned() }))
}

/// 安卓：把安装器复制到 Download/LeafStarter/，方便在启动器里选
fn save_installer_public(inst: &Path) -> Option<String> {
    let dir = Path::new(crate::game::launchers::ANDROID_STORAGE).join("Download").join("LeafStarter");
    std::fs::create_dir_all(&dir).ok()?;
    let dest = dir.join(inst.file_name()?);
    std::fs::copy(inst, &dest).ok()?;
    Some(dest.to_string_lossy().into_owned())
}
