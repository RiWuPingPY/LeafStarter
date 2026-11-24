//! 平台相关：安卓通过 Kotlin 插件（McBridgePlugin）访问包管理器和系统文件授权；桌面端是空实现。

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::game::install::Env;
use crate::game::launchers::{InstalledApp, SafGrant, Target};
use crate::game::mojang::InstalledVersion;
use crate::base::task::TaskEntry;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PickResult {
    pub uri: String,
    pub rel: String,
    pub label: String,
    pub found: bool,
}

#[cfg(target_os = "android")]
mod imp {
    use super::*;
    use anyhow::{anyhow, bail};
    use serde_json::{json, Value};
    use tauri::plugin::{PluginHandle, TauriPlugin};
    use tauri::{AppHandle, Manager, Wry};

    pub struct Bridge(pub PluginHandle<Wry>);

    pub fn plugin() -> TauriPlugin<Wry> {
        tauri::plugin::Builder::new("mcbridge")
            .setup(|app, api| {
                let handle = api.register_android_plugin("com.leafhub.leafstarter", "McBridgePlugin")?;
                app.manage(Bridge(handle));
                Ok(())
            })
            .build()
    }

    fn handle(app: &AppHandle) -> Result<PluginHandle<Wry>> {
        let b = app.try_state::<Bridge>().ok_or_else(|| anyhow!("安卓桥接插件没有加载"))?;
        Ok(b.0.clone())
    }

    async fn call(app: &AppHandle, method: &'static str, payload: Value) -> Result<Value> {
        let h = handle(app)?;
        tokio::task::spawn_blocking(move || h.run_mobile_plugin::<Value>(method, payload))
            .await?
            .map_err(|e| anyhow!("{}", e))
    }

    pub async fn installed_apps(app: &AppHandle) -> Result<Vec<InstalledApp>> {
        let pkgs: Vec<&str> = crate::game::launchers::ANDROID_LAUNCHERS.iter().map(|(p, _, _)| *p).collect();
        let v = call(app, "installedApps", json!({ "packages": pkgs })).await?;
        Ok(serde_json::from_value(v.get("apps").cloned().unwrap_or(Value::Array(vec![])))?)
    }

    pub async fn all_files_access(app: &AppHandle) -> bool {
        call(app, "allFilesAccess", json!({})).await.ok().and_then(|v| v.get("granted").and_then(|g| g.as_bool())).unwrap_or(false)
    }

    pub async fn request_all_files(app: &AppHandle) -> Result<()> {
        call(app, "requestAllFilesAccess", json!({})).await.map(|_| ())
    }

    pub async fn pick_tree(app: &AppHandle, package: Option<String>) -> Result<PickResult> {
        let initial = package.map(|p| format!("content://{0}.document.provider/document/{0}", p));
        let v = call(app, "pickTree", json!({ "initialUri": initial })).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn saf_versions(app: &AppHandle, grant: &SafGrant) -> Vec<InstalledVersion> {
        let path = if grant.rel.is_empty() { "versions".to_string() } else { format!("{}/versions", grant.rel) };
        let Ok(v) = call(app, "listDirs", json!({ "uri": grant.uri, "path": path })).await else { return vec![] };
        v.get("names")
            .and_then(|n| n.as_array())
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str())
            .map(|n| InstalledVersion { name: n.to_string(), ..Default::default() })
            .collect()
    }

    pub async fn release_tree(app: &AppHandle, uri: &str) {
        let _ = call(app, "releaseTree", json!({ "uri": uri })).await;
    }

    pub async fn copy_to_saf(env: &Env, target: &Target, root: &Path, files: &[String], task: &TaskEntry) -> Result<()> {
        let app = env.app.as_ref().ok_or_else(|| anyhow!("没有界面环境"))?;
        let uri = target.saf_uri.clone().ok_or_else(|| anyhow!("目标没有授权信息"))?;
        let rel = target.saf_rel.clone().unwrap_or_default();
        task.reset_counters();
        task.prog.files_total.store(files.len() as u64, std::sync::atomic::Ordering::Relaxed);
        let mut copied = 0u64;
        let mut skipped = 0u64;
        // 分批调用，批与批之间更新进度、检查取消
        for (i, chunk) in files.chunks(200).enumerate() {
            task.prog.check()?;
            task.stage(format!("写入「{}」（{}/{}）", target.launcher, (i * 200).min(files.len()), files.len()));
            let v = call(
                app,
                "copyIn",
                json!({ "uri": uri, "rel": rel, "srcRoot": root.to_string_lossy(), "files": chunk }),
            )
            .await?;
            copied += v.get("copied").and_then(|c| c.as_u64()).unwrap_or(0);
            skipped += v.get("skipped").and_then(|c| c.as_u64()).unwrap_or(0);
            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                bail!("写入启动器目录失败：{}", err);
            }
            task.prog.files_done.fetch_add(chunk.len() as u64, std::sync::atomic::Ordering::Relaxed);
        }
        task.log(format!("写入 {} 个文件，跳过 {} 个已存在的", copied, skipped));
        Ok(())
    }

    pub fn open_path(_path: &str) -> Result<()> {
        bail!("安卓上不支持打开文件夹")
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    use super::*;
    use anyhow::bail;
    use tauri::AppHandle;

    pub async fn installed_apps(_app: &AppHandle) -> Result<Vec<InstalledApp>> {
        Ok(vec![])
    }
    pub async fn all_files_access(_app: &AppHandle) -> bool {
        true
    }
    pub async fn request_all_files(_app: &AppHandle) -> Result<()> {
        Ok(())
    }
    pub async fn pick_tree(_app: &AppHandle, _package: Option<String>) -> Result<PickResult> {
        bail!("仅安卓需要授权目录")
    }
    pub async fn saf_versions(_app: &AppHandle, _grant: &SafGrant) -> Vec<InstalledVersion> {
        vec![]
    }
    pub async fn release_tree(_app: &AppHandle, _uri: &str) {}
    pub async fn copy_to_saf(_env: &Env, _target: &Target, _root: &Path, _files: &[String], _task: &TaskEntry) -> Result<()> {
        bail!("仅安卓支持")
    }

    pub fn open_path(path: &str) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new("explorer").arg(path).creation_flags(0x0800_0000).spawn()?;
        }
        #[cfg(target_os = "macos")]
        std::process::Command::new("open").arg(path).spawn()?;
        #[cfg(all(unix, not(target_os = "macos")))]
        std::process::Command::new("xdg-open").arg(path).spawn()?;
        Ok(())
    }
}

pub use imp::*;
