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
    // 记下整合包自带的每个文件（以后更新时分得清哪些是玩家自己改的、自己加的）
    if let Ok(m) = manifest_of(plan, zip) {
        let _ = save_manifest(&gd, &m);
    }
    let mut cfg = crate::instance::load_config(&root, name);
    cfg.pack = Some(json!({ "format": plan.format, "name": plan.name, "version": plan.version }));
    crate::instance::save_config(&root, name, &cfg)?;
    let mut msg = format!("装好了：{} 个文件，{n} 个配置", plan.files.len());
    if !plan.manual.is_empty() {
        msg.push_str(&format!("。还有 {} 个文件作者不让第三方下载，要自己到 CurseForge 下好放进 mods：{}", plan.manual.len(), plan.manual.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join("、")));
    }
    Ok(Some(msg))
}

// ---------- 整合包更新 ----------
// 装的时候记下整合包自带的文件（相对游戏目录的路径 → SHA-1）。更新时：
// - 整合包的文件：本地还是原样的就按新版本换（新版本没有的删掉）；玩家换过的不动
// - 配置（overrides）：本地还是原样的换成新的；玩家改过的保留，新的另存成「文件名.new」给玩家自己对
// - 玩家自己加的 Mod、存档、截图：一概不碰
// 换下来、删掉的旧文件先挪进 LeafStarter/pack-backup/<旧版本>/，后悔了能拿回来

#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
pub struct Manifest {
    pub version: String,
    /// 下载的文件：路径 → SHA-1
    pub files: std::collections::BTreeMap<String, String>,
    /// overrides 里解出来的：路径 → SHA-1
    pub overrides: std::collections::BTreeMap<String, String>,
}

fn manifest_path(gd: &Path) -> PathBuf {
    gd.join("LeafStarter").join("pack-files.json")
}

pub fn load_manifest(gd: &Path) -> Option<Manifest> {
    serde_json::from_slice(&std::fs::read(manifest_path(gd)).ok()?).ok()
}

pub fn save_manifest(gd: &Path, m: &Manifest) -> Result<()> {
    crate::base::util::write_json(&manifest_path(gd), &serde_json::to_value(m)?)
}

fn sha1_hex(data: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

fn norm(p: &str) -> String {
    p.replace('\\', "/")
}

/// 从整合包本身算出清单（下载的文件用索引里的 SHA-1，overrides 现算）
pub fn manifest_of(plan: &Plan, zip: &Path) -> Result<Manifest> {
    let mut m = Manifest { version: plan.version.clone(), ..Default::default() };
    for f in &plan.files {
        if let Some(s) = &f.sha1 {
            m.files.insert(norm(&f.path), s.to_lowercase());
        }
    }
    let mut z = zip::ZipArchive::new(std::io::BufReader::new(std::fs::File::open(zip)?))?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = norm(e.name());
        let Some(rel) = plan.overrides.iter().find_map(|p| name.strip_prefix(p.as_str())) else { continue };
        if rel.is_empty() || crate::sync::store::safe_join(Path::new("."), rel).is_err() {
            continue;
        }
        let rel = rel.to_string();
        let mut buf = vec![];
        e.read_to_end(&mut buf)?;
        m.overrides.insert(rel, sha1_hex(&buf));
    }
    Ok(m)
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSummary {
    pub from: String,
    pub to: String,
    pub added: usize,
    pub replaced: usize,
    pub removed: usize,
    /// 玩家改过、没动的配置（新版本的另存成 .new）
    pub kept: Vec<String>,
    pub backup: String,
}

fn local_sha1(p: &Path) -> Option<String> {
    crate::base::net::sha1_file(p).ok().map(|s| s.to_lowercase())
}

/// 把旧文件挪进备份文件夹（同一个相对路径）
fn backup(gd: &Path, bk: &Path, rel: &str) -> Result<()> {
    let src = crate::sync::store::safe_join(gd, rel)?;
    if !src.is_file() {
        return Ok(());
    }
    let dst = crate::sync::store::safe_join(bk, rel)?;
    std::fs::create_dir_all(dst.parent().unwrap())?;
    crate::base::util::move_file(&src, &dst)?;
    Ok(())
}

/// 按新版本的整合包更新已经装好的实例（游戏和加载器版本变了的，调用方先重装版本）
pub async fn update(env: &Env, gd: &Path, old: &Manifest, plan: &Plan, zip: &Path, task: &Arc<TaskEntry>) -> Result<UpdateSummary> {
    let new = manifest_of(plan, zip)?;
    let stamp = if old.version.is_empty() { crate::base::util::now_ms().to_string() } else { old.version.clone() };
    let bk = gd.join("LeafStarter").join("pack-backup").join(crate::content::install::safe_name(&stamp).unwrap_or_else(|_| "旧版本".into()));
    let mut s = UpdateSummary { from: old.version.clone(), to: plan.version.clone(), backup: bk.to_string_lossy().into_owned(), ..Default::default() };

    // 1. 下载的文件
    task.stage("对比文件");
    let mut jobs = vec![];
    for f in &plan.files {
        let rel = norm(&f.path);
        let dst = crate::sync::store::safe_join(gd, &rel)?;
        let want = f.sha1.clone().unwrap_or_default().to_lowercase();
        let have = if dst.is_file() { local_sha1(&dst) } else { None };
        if have.as_deref() == Some(want.as_str()) {
            continue;
        }
        match (&have, old.files.get(&rel)) {
            // 本地有、和旧版本记的不一样：玩家换过，不动
            (Some(h), Some(o)) if h != o => continue,
            (Some(_), _) => {
                backup(gd, &bk, &rel)?;
                s.replaced += 1;
            }
            (None, _) => s.added += 1,
        }
        jobs.push(FileJob { url: f.url.clone(), path: dst, sha1: f.sha1.clone(), size: f.size });
    }
    for (rel, sha) in &old.files {
        if new.files.contains_key(rel) {
            continue;
        }
        let p = crate::sync::store::safe_join(gd, rel)?;
        if p.is_file() && local_sha1(&p).as_deref() == Some(sha.as_str()) {
            backup(gd, &bk, rel)?;
            s.removed += 1;
        }
    }
    if !jobs.is_empty() {
        task.stage(format!("下载 {} 个文件", jobs.len()));
        task.reset_counters();
        install::download_all(&env.net, &jobs, &task.prog).await?;
    }

    // 2. 配置
    task.stage("更新配置");
    let mut z = zip::ZipArchive::new(std::io::BufReader::new(std::fs::File::open(zip)?))?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = norm(e.name());
        let Some(rel) = plan.overrides.iter().find_map(|p| name.strip_prefix(p.as_str())).map(str::to_string) else { continue };
        let Some(want) = new.overrides.get(&rel) else { continue };
        let dst = crate::sync::store::safe_join(gd, &rel)?;
        let have = if dst.is_file() { local_sha1(&dst) } else { None };
        if have.as_deref() == Some(want.as_str()) {
            continue;
        }
        let mut buf = vec![];
        e.read_to_end(&mut buf)?;
        let user_changed = match (&have, old.overrides.get(&rel)) {
            (Some(h), Some(o)) => h != o,
            // 旧版本没有这个文件、本地却有：玩家自己放的
            (Some(_), None) => true,
            (None, _) => false,
        };
        std::fs::create_dir_all(dst.parent().unwrap())?;
        if user_changed {
            let mut alt = dst.clone().into_os_string();
            alt.push(".new");
            std::fs::write(PathBuf::from(alt), &buf)?;
            s.kept.push(rel.clone());
        } else {
            if have.is_some() {
                backup(gd, &bk, &rel)?;
                s.replaced += 1;
            } else {
                s.added += 1;
            }
            std::fs::write(&dst, &buf)?;
        }
    }
    for (rel, sha) in &old.overrides {
        if new.overrides.contains_key(rel) {
            continue;
        }
        let p = crate::sync::store::safe_join(gd, rel)?;
        if p.is_file() && local_sha1(&p).as_deref() == Some(sha.as_str()) {
            backup(gd, &bk, rel)?;
            s.removed += 1;
        }
    }
    save_manifest(gd, &new)?;
    Ok(s)
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

    fn zip_with(path: &Path, files: &[(&str, &str)]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, c) in files {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, c.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn plan_v(v: &str) -> Plan {
        Plan { format: "mrpack".into(), name: "测试包".into(), version: v.into(), mc: "1.20.1".into(), loader: None, files: vec![], manual: vec![], overrides: vec!["overrides/".into()] }
    }

    #[tokio::test]
    async fn update_keeps_what_player_changed() {
        let t = std::env::temp_dir().join(format!("ls-packupd-{}", crate::base::util::now_ms()));
        let gd = t.join("game");
        std::fs::create_dir_all(&gd).unwrap();
        let (z1, z2) = (t.join("v1.zip"), t.join("v2.zip"));
        zip_with(&z1, &[("overrides/config/a.txt", "A1"), ("overrides/config/b.txt", "B1"), ("overrides/config/d.txt", "D1")]);
        zip_with(&z2, &[("overrides/config/a.txt", "A2"), ("overrides/config/b.txt", "B2"), ("overrides/config/c.txt", "C2")]);
        let (p1, p2) = (plan_v("1.0"), plan_v("2.0"));
        extract(&z1, &p1.overrides, &gd).unwrap();
        let old = manifest_of(&p1, &z1).unwrap();
        // 玩家改了 b，自己放了 mods/mine.jar
        std::fs::write(gd.join("config/b.txt"), "mine").unwrap();
        std::fs::create_dir_all(gd.join("mods")).unwrap();
        std::fs::write(gd.join("mods/mine.jar"), "x").unwrap();
        let env = Env { app: None, net: Arc::new(crate::base::net::Net::new(Default::default()).unwrap()), data_dir: t.clone() };
        let task = crate::base::task::Tasks::default().create("t".into(), "t".into(), String::new());
        let s = update(&env, &gd, &old, &p2, &z2, &task).await.unwrap();
        let read = |r: &str| std::fs::read_to_string(gd.join(r)).ok();
        assert_eq!(read("config/a.txt").as_deref(), Some("A2"), "没改过的换新");
        assert_eq!(read("config/b.txt").as_deref(), Some("mine"), "改过的不动");
        assert_eq!(read("config/b.txt.new").as_deref(), Some("B2"), "新的另存");
        assert_eq!(read("config/c.txt").as_deref(), Some("C2"), "新加的");
        assert!(read("config/d.txt").is_none(), "新版本没有的删掉");
        assert_eq!(read("mods/mine.jar").as_deref(), Some("x"), "自己加的 Mod 不碰");
        assert_eq!((s.added, s.replaced, s.removed, s.kept.len()), (1, 1, 1, 1));
        assert!(Path::new(&s.backup).join("config/d.txt").is_file() && Path::new(&s.backup).join("config/a.txt").is_file(), "旧文件进了备份");
        assert_eq!(load_manifest(&gd).unwrap().version, "2.0");
        let _ = std::fs::remove_dir_all(&t);
    }

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
