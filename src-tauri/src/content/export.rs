//! 导出成 Modrinth 整合包（.mrpack，ROADMAP 5.9）。
//! Modrinth 上认得的 Mod 只写下载地址（别人装的时候自己去 Modrinth 下，文件小、也合规），
//! 认不出的和配置文件一起放进 overrides。存档默认不带。

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;
use sha2::{Digest, Sha512};

use super::modrinth;

const OVERRIDES: &[&str] = &["config", "defaultconfigs", "kubejs", "scripts", "resourcepacks", "shaderpacks", "options.txt", "servers.dat"];

pub struct Summary {
    pub linked: usize,
    pub bundled: usize,
}

#[allow(clippy::too_many_arguments)]
pub async fn export(gd: &Path, out: &Path, name: &str, version: &str, mc: &str, loader: Option<(&str, &str)>, saves: bool) -> Result<Summary> {
    let http = modrinth::client()?;
    // mods 里启用着的文件
    let mods: Vec<std::path::PathBuf> = std::fs::read_dir(gd.join("mods")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "jar")).collect();
    let mut hashes = vec![];
    for p in &mods {
        hashes.push(crate::sync::store::sha1_file(p)?);
    }
    let known = modrinth::by_hashes(&http, &hashes).await.unwrap_or_default();
    let mut files = vec![];
    let mut bundle: Vec<std::path::PathBuf> = vec![];
    for (p, h) in mods.iter().zip(&hashes) {
        let fname = p.file_name().unwrap().to_string_lossy().into_owned();
        match known.get(h).and_then(|v| v.files.iter().find(|f| &f.sha1 == h).cloned()) {
            Some(f) => {
                let bytes = std::fs::read(p)?;
                let sha512: String = Sha512::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
                files.push(json!({ "path": format!("mods/{fname}"), "hashes": { "sha1": h, "sha512": sha512 }, "env": { "client": "required", "server": "required" }, "downloads": [f.url], "fileSize": bytes.len() }));
            }
            None => bundle.push(p.clone()),
        }
    }
    let mut deps = json!({ "minecraft": mc });
    if let Some((l, v)) = loader {
        let key = match l.to_lowercase().as_str() {
            "fabric" => "fabric-loader",
            "quilt" => "quilt-loader",
            "forge" => "forge",
            "neoforge" => "neoforge",
            _ => "",
        };
        if !key.is_empty() {
            deps[key] = json!(v);
        }
    }
    let index = json!({ "formatVersion": 1, "game": "minecraft", "versionId": version, "name": name, "summary": "用 LeafStarter 导出", "files": files, "dependencies": deps });

    let tmp = out.with_extension("mrpack.ls-tmp");
    let f = std::fs::File::create(&tmp).with_context(|| format!("写不了 {}", out.display()))?;
    let mut z = zip::ZipWriter::new(f);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("modrinth.index.json", opt)?;
    z.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
    let mut add = |abs: &Path, rel: &str| -> Result<()> {
        z.start_file(format!("overrides/{rel}"), opt)?;
        z.write_all(&std::fs::read(abs)?)?;
        Ok(())
    };
    for p in &bundle {
        add(p, &format!("mods/{}", p.file_name().unwrap().to_string_lossy()))?;
    }
    let mut extra: Vec<&str> = OVERRIDES.to_vec();
    if saves {
        extra.push("saves");
    }
    for item in extra {
        let p = gd.join(item);
        if p.is_file() {
            add(&p, item)?;
        } else if p.is_dir() {
            let mut stack = vec![p.clone()];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d)?.flatten() {
                    let ep = e.path();
                    if ep.is_dir() {
                        stack.push(ep);
                    } else if ep.file_name().is_some_and(|n| n != "session.lock") {
                        let rel = ep.strip_prefix(gd).unwrap().to_string_lossy().replace('\\', "/");
                        add(&ep, &rel)?;
                    }
                }
            }
        }
    }
    z.finish()?;
    crate::base::util::move_file(&tmp, out)?;
    Ok(Summary { linked: files.len(), bundled: bundle.len() })
}
