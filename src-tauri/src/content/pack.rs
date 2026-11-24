//! 整合包导入（ROADMAP 5.9、M8）：Modrinth `.mrpack`、CurseForge zip、MultiMC / Prism、MCBBS / HMCL。
//! 先认出格式读成统一的计划（游戏版本、加载器、要下载的文件、要解压的 overrides），再按计划装成一个新实例。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::base::net::FileJob;
use crate::base::task::TaskEntry;
use crate::game::install::{self, Env, InstallRequest, LoaderReq};
use crate::game::launchers::Target;
use crate::game::loaders::{self, LoaderKind};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    /// 相对游戏目录，比如 mods/sodium.jar
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// mrpack / curseforge / multimc / mcbbs
    pub format: String,
    pub name: String,
    pub version: String,
    pub mc: String,
    /// (加载器, 版本)
    pub loader: Option<(String, String)>,
    pub files: Vec<PackFile>,
    /// 下载不了、要用户自己下的（CurseForge 作者关了第三方下载）：(文件名, 项目页)
    pub manual: Vec<(String, String)>,
    /// zip 里要原样解压到游戏目录的前缀，比如 overrides/
    pub overrides: Vec<String>,
}

fn read_text<R: Read + std::io::Seek>(z: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let mut f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s.trim_start_matches('\u{feff}').to_string())
}

/// 认格式、读计划。CurseForge 的文件要问代理拿下载地址
pub async fn read(path: &Path) -> Result<Plan> {
    let f = std::fs::File::open(path).with_context(|| format!("打不开 {}", path.display()))?;
    let mut z = zip::ZipArchive::new(std::io::BufReader::new(f)).context("这不是整合包（不是 zip 文件）")?;
    if let Some(t) = read_text(&mut z, "modrinth.index.json") {
        return mrpack(&t);
    }
    if let Some(t) = read_text(&mut z, "manifest.json") {
        let v: Value = serde_json::from_str(&t).context("manifest.json 读不懂")?;
        if v["manifestType"] == "minecraftModpack" {
            return curseforge(&v).await;
        }
        if v.get("addons").is_some() {
            return mcbbs(&v);
        }
    }
    if let Some(t) = read_text(&mut z, "mcbbs.packmeta") {
        return mcbbs(&serde_json::from_str(&t).context("mcbbs.packmeta 读不懂")?);
    }
    // MultiMC / Prism：mmc-pack.json 可能在根目录，也可能在一层文件夹里
    let names: Vec<String> = z.file_names().map(String::from).collect();
    if let Some(p) = names.iter().find(|n| n.ends_with("mmc-pack.json") && n.matches('/').count() <= 1) {
        let prefix = p.trim_end_matches("mmc-pack.json").to_string();
        let t = read_text(&mut z, p).unwrap_or_default();
        let cfg = read_text(&mut z, &format!("{prefix}instance.cfg")).unwrap_or_default();
        return multimc(&serde_json::from_str(&t).context("mmc-pack.json 读不懂")?, &cfg, &prefix, &names);
    }
    bail!("认不出这个整合包的格式。支持 Modrinth（.mrpack）、CurseForge、MultiMC / Prism、MCBBS / HMCL")
}

fn mrpack(t: &str) -> Result<Plan> {
    let v: Value = serde_json::from_str(t).context("modrinth.index.json 读不懂")?;
    let deps = &v["dependencies"];
    let mc = deps["minecraft"].as_str().context("整合包没写游戏版本")?.to_string();
    let loader = [("fabric-loader", "Fabric"), ("quilt-loader", "Quilt"), ("forge", "Forge"), ("neoforge", "NeoForge")]
        .iter()
        .find_map(|(k, n)| deps[*k].as_str().map(|ver| (n.to_string(), ver.to_string())));
    let mut files = vec![];
    for f in v["files"].as_array().into_iter().flatten() {
        // 只装客户端要的（env.client 是 unsupported 的跳过）
        if f["env"]["client"] == "unsupported" {
            continue;
        }
        let path = f["path"].as_str().context("文件没有路径")?.to_string();
        crate::sync::store::safe_join(Path::new("."), &path)?;
        let url = f["downloads"][0].as_str().context("文件没有下载地址")?.to_string();
        files.push(PackFile { path, url, sha1: f["hashes"]["sha1"].as_str().map(String::from), size: f["fileSize"].as_u64() });
    }
    Ok(Plan {
        format: "mrpack".into(),
        name: v["name"].as_str().unwrap_or("整合包").into(),
        version: v["versionId"].as_str().unwrap_or("").into(),
        mc,
        loader,
        files,
        manual: vec![],
        overrides: vec!["overrides/".into(), "client-overrides/".into()],
    })
}

async fn curseforge(v: &Value) -> Result<Plan> {
    let mc = v["minecraft"]["version"].as_str().context("整合包没写游戏版本")?.to_string();
    let loader = v["minecraft"]["modLoaders"].as_array().and_then(|a| a.iter().find(|l| l["primary"].as_bool().unwrap_or(true)).or(a.first())).and_then(|l| l["id"].as_str()).and_then(|id| {
        let (k, ver) = id.split_once('-')?;
        let kind = match k {
            "forge" => "Forge",
            "neoforge" => "NeoForge",
            "fabric" => "Fabric",
            "quilt" => "Quilt",
            _ => return None,
        };
        // NeoForge 1.20.1 的写法是 neoforge-1.20.1-47.1.x
        let ver = if kind == "NeoForge" && ver.starts_with("1.20.1-") { ver.trim_start_matches("1.20.1-").to_string() } else { ver.to_string() };
        Some((kind.to_string(), ver))
    });
    let ids: Vec<u64> = v["files"].as_array().into_iter().flatten().filter(|f| f["required"].as_bool().unwrap_or(true)).filter_map(|f| f["fileID"].as_u64()).collect();
    let mut files = vec![];
    let mut manual = vec![];
    if !ids.is_empty() {
        if super::curseforge::proxy().is_none() {
            bail!("这是 CurseForge 整合包，要先接上 CurseForge 才能下里面的 {} 个文件（代理还没部署）", ids.len());
        }
        let infos = super::curseforge::files(&ids).await?;
        let classes = super::curseforge::classes(&infos.iter().map(|f| f.0).collect::<Vec<_>>()).await.unwrap_or_default();
        for (mod_id, name, url, sha1, size) in infos {
            let folder = match classes.get(&mod_id).copied() {
                Some(12) => "resourcepacks",
                Some(6552) => "shaderpacks",
                _ => "mods",
            };
            if url.is_empty() {
                manual.push((name, format!("https://www.curseforge.com/projects/{mod_id}")));
            } else {
                files.push(PackFile { path: format!("{folder}/{name}"), url, sha1, size });
            }
        }
    }
    Ok(Plan {
        format: "curseforge".into(),
        name: v["name"].as_str().unwrap_or("整合包").into(),
        version: v["version"].as_str().unwrap_or("").into(),
        mc,
        loader,
        files,
        manual,
        overrides: vec![format!("{}/", v["overrides"].as_str().unwrap_or("overrides"))],
    })
}

fn mcbbs(v: &Value) -> Result<Plan> {
    let addons = v["addons"].as_array().cloned().unwrap_or_default();
    let get = |id: &str| addons.iter().find(|a| a["id"] == id).and_then(|a| a["version"].as_str()).map(String::from);
    let mc = get("game").context("整合包没写游戏版本")?;
    let loader = [("forge", "Forge"), ("neoforge", "NeoForge"), ("fabric", "Fabric"), ("quilt", "Quilt")].iter().find_map(|(k, n)| get(k).map(|ver| (n.to_string(), ver)));
    Ok(Plan { format: "mcbbs".into(), name: v["name"].as_str().unwrap_or("整合包").into(), version: v["version"].as_str().unwrap_or("").into(), mc, loader, files: vec![], manual: vec![], overrides: vec!["overrides/".into()] })
}

fn multimc(v: &Value, cfg: &str, prefix: &str, names: &[String]) -> Result<Plan> {
    let comps = v["components"].as_array().cloned().unwrap_or_default();
    let get = |uid: &str| comps.iter().find(|c| c["uid"] == uid).and_then(|c| c["version"].as_str()).map(String::from);
    let mc = get("net.minecraft").context("整合包没写游戏版本")?;
    let loader = [("net.fabricmc.fabric-loader", "Fabric"), ("org.quiltmc.quilt-loader", "Quilt"), ("net.minecraftforge", "Forge"), ("net.neoforged", "NeoForge")].iter().find_map(|(k, n)| get(k).map(|ver| (n.to_string(), ver)));
    let name = cfg.lines().find_map(|l| l.strip_prefix("name=")).unwrap_or("整合包").to_string();
    let game = [".minecraft/", "minecraft/"].iter().map(|d| format!("{prefix}{d}")).find(|p| names.iter().any(|n| n.starts_with(p.as_str()))).unwrap_or_else(|| format!("{prefix}.minecraft/"));
    Ok(Plan { format: "multimc".into(), name, version: String::new(), mc, loader, files: vec![], manual: vec![], overrides: vec![game] })
}

/// 按计划装成一个新实例：先装游戏和加载器，再下文件、解压 overrides，最后记下来自哪个整合包
pub async fn install(env: &Env, target: Target, plan: &Plan, zip: &Path, name: &str, task: Arc<TaskEntry>) -> Result<Option<String>> {
    let root = PathBuf::from(&target.path);
    let loader = match &plan.loader {
        Some((kind, ver)) => {
            let k = LoaderKind::parse(kind).with_context(|| format!("不支持的加载器 {kind}"))?;
            task.stage(format!("找 {kind} {ver}"));
            let list = loaders::list(&env.net, k, &plan.mc).await?;
            let v = list.into_iter().find(|x| &x.version == ver || x.id.contains(ver.as_str())).with_context(|| format!("找不到 {kind} {ver}"))?;
            Some(LoaderReq { kind: k.label().into(), id: v.id, version: v.version })
        }
        None => None,
    };
    install::run(env, target.clone(), InstallRequest { target_id: target.id.clone(), mc_version: plan.mc.clone(), name: name.to_string(), loader, overwrite: false }, task.clone()).await?;
    // 整合包一律版本隔离：Mod、配置、存档放在版本文件夹里
    let gd = root.join("versions").join(name);
    std::fs::create_dir_all(gd.join("mods"))?;

    task.stage(format!("下载整合包里的 {} 个文件", plan.files.len()));
    task.reset_counters();
    let jobs: Vec<FileJob> = plan.files.iter().map(|f| Ok(FileJob { url: f.url.clone(), path: crate::sync::store::safe_join(&gd, &f.path)?, sha1: f.sha1.clone(), size: f.size })).collect::<Result<_>>()?;
    install::download_all(&env.net, &jobs, &task.prog).await?;

    task.stage("解压配置文件");
    let n = extract(zip, &plan.overrides, &gd)?;
    let mut cfg = crate::instance::load_config(&root, name);
    cfg.pack = Some(json!({ "format": plan.format, "name": plan.name, "version": plan.version }));
    crate::instance::save_config(&root, name, &cfg)?;
    let mut msg = format!("装好了：{} 个文件，{n} 个配置", plan.files.len());
    if !plan.manual.is_empty() {
        msg.push_str(&format!("。还有 {} 个文件作者不让第三方下载，要自己到 CurseForge 下好放进 mods：{}", plan.manual.len(), plan.manual.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join("、")));
    }
    Ok(Some(msg))
}

/// 把 zip 里这些前缀下的文件解压到游戏目录（路径不许跑出去）
fn extract(zip: &Path, prefixes: &[String], gd: &Path) -> Result<usize> {
    let f = std::fs::File::open(zip)?;
    let mut z = zip::ZipArchive::new(std::io::BufReader::new(f))?;
    let mut n = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().replace('\\', "/");
        let Some(rel) = prefixes.iter().find_map(|p| name.strip_prefix(p.as_str())) else { continue };
        if rel.is_empty() {
            continue;
        }
        let dst = crate::sync::store::safe_join(gd, rel)?;
        std::fs::create_dir_all(dst.parent().unwrap())?;
        let mut out = std::fs::File::create(&dst).with_context(|| format!("写不了 {rel}"))?;
        std::io::copy(&mut e, &mut out)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mrpack_index() {
        let p = mrpack(r#"{"formatVersion":1,"game":"minecraft","versionId":"5.0","name":"Fabulously","files":[
            {"path":"mods/sodium.jar","hashes":{"sha1":"aa","sha512":"bb"},"downloads":["https://cdn.modrinth.com/x/sodium.jar"],"fileSize":10},
            {"path":"mods/server-only.jar","hashes":{"sha1":"cc"},"downloads":["https://x/y.jar"],"env":{"client":"unsupported","server":"required"}}],
            "dependencies":{"minecraft":"1.20.1","fabric-loader":"0.15.7"}}"#).unwrap();
        assert_eq!((p.mc.as_str(), p.loader.clone()), ("1.20.1", Some(("Fabric".to_string(), "0.15.7".to_string()))));
        assert_eq!(p.files.len(), 1, "只给服务端的不装");
        assert!(mrpack(r#"{"files":[{"path":"../evil.jar","downloads":["https://x"]}],"dependencies":{"minecraft":"1.20.1"}}"#).is_err(), "路径跑出去的拒绝");
    }

    #[test]
    fn reads_multimc() {
        let v: Value = serde_json::from_str(r#"{"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.minecraftforge","version":"47.2.0"}]}"#).unwrap();
        let p = multimc(&v, "InstanceType=OneSix\nname=我的包\n", "pack/", &["pack/.minecraft/mods/a.jar".to_string()]).unwrap();
        assert_eq!((p.name.as_str(), p.loader.clone(), p.overrides.clone()), ("我的包", Some(("Forge".to_string(), "47.2.0".to_string())), vec!["pack/.minecraft/".to_string()]));
    }
}
