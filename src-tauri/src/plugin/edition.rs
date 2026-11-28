//! 定制版（M14，ROADMAP 5.18）：服主和整合包作者给自己的玩家发「XX 服专用启动器」。
//! 一个签名的定制包（`.lsedition`，zip）：`edition.json` + 可选的 `icon.png`、`splash.png`（启动画面）、
//! `pack.mrpack`、`plugins/*.lsplugin`（带上的第三方插件，各自也要有作者签名）。
//! 放在 LeafStarter.exe 旁边（便携版）或者导入安装版（存在 `<数据目录>/edition/current.lsedition`）。
//! exe 还是我们签名的那一个；「基于 LeafStarter」由程序固定显示，定制包里关不掉。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::sign;

pub const EXT: &str = "lsedition";

/// 读出来的定制包
#[derive(Debug, Clone)]
pub struct Edition {
    pub manifest: Value,
    pub key: Vec<u8>,
    pub path: PathBuf,
    pub icon: Option<Vec<u8>>,
    pub splash: Option<Vec<u8>>,
    pub has_pack: bool,
    /// 带上的插件：文件名
    pub plugins: Vec<String>,
}

impl Edition {
    pub fn view(&self) -> Value {
        let mut m = self.manifest.clone();
        m["fingerprint"] = json!(sign::fingerprint(&self.key));
        m["path"] = json!(self.path.to_string_lossy());
        m["icon"] = json!(self.icon.as_ref().map(|b| format!("data:image/png;base64,{}", sign::b64(b))));
        m["splash"] = json!(self.splash.as_ref().map(|b| format!("data:image/png;base64,{}", sign::b64(b))));
        m["plugins"] = json!(self.plugins);
        m["hasPack"] = json!(self.has_pack);
        m
    }
}

/// 检查清单：只留认得的字段，长度有限
pub fn clean(m: &Value) -> Result<Value> {
    if m["format"].as_str() != Some("leafstarter-edition/1") {
        bail!("不认识这个定制包的格式");
    }
    let s = |v: &Value, n: usize| v.as_str().map(|x| x.chars().take(n).collect::<String>()).unwrap_or_default();
    let name = s(&m["name"], 30);
    if name.trim().is_empty() {
        bail!("定制包没有名字");
    }
    let color = s(&m["theme"]["color"], 7);
    let color = if color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit()) { color } else { "#0E7C86".into() };
    let feats = ["download", "ai", "skin", "world", "mp", "sync"];
    let features: serde_json::Map<String, Value> = feats.iter().map(|k| (k.to_string(), json!(m["features"][*k].as_bool().unwrap_or(true)))).collect();
    let servers: Vec<Value> = m["servers"].as_array().map(|a| a.iter().take(10).map(|x| json!({ "name": s(&x["name"], 30), "address": s(&x["address"], 100) })).filter(|x| !x["address"].as_str().unwrap_or("").is_empty()).collect()).unwrap_or_default();
    let update = s(&m["update"], 300);
    if !update.is_empty() && !update.starts_with("https://") {
        bail!("更新地址要用 https");
    }
    Ok(json!({
        "format": "leafstarter-edition/1",
        "name": name, "author": s(&m["author"], 40), "version": s(&m["version"], 20),
        "theme": { "color": color, "font": if m["theme"]["font"] == "serif" { "serif" } else { "sans" } },
        "features": features, "minimal": m["minimal"].as_bool().unwrap_or(false),
        "pack": {
            "name": s(&m["pack"]["name"], 40), "mc": s(&m["pack"]["mc"], 30), "loader": s(&m["pack"]["loader"], 20),
            "modrinth": s(&m["pack"]["modrinth"], 80),
        },
        "servers": servers,
        "home": { "notice": s(&m["home"]["notice"], 500) },
        "update": update,
        // 锁住的设置：玩家改不了（服主配好的东西不被改坏）
        "lock": LOCKS.iter().filter(|k| m["lock"].as_array().is_some_and(|a| a.iter().any(|x| x == **k))).collect::<Vec<_>>(),
    }))
}

/// 能锁的设置：主题、下载源和代理、Java、新建 / 删除实例、实例的内存和启动参数
pub const LOCKS: [&str; 5] = ["theme", "sources", "java", "instances", "launchArgs"];

/// 读并校验一个定制包
pub fn load(path: &Path) -> Result<Edition> {
    let files = sign::read_zip(path)?;
    let key = sign::check(&files)?;
    let raw: Value = serde_json::from_slice(files.get("edition.json").context("定制包里没有 edition.json")?).context("edition.json 坏了")?;
    let manifest = clean(&raw)?;
    let icon = files.get("icon.png").filter(|b| b.starts_with(b"\x89PNG") && b.len() < 512 * 1024).cloned();
    let splash = files.get("splash.png").filter(|b| b.starts_with(b"\x89PNG") && b.len() < 2 * 1024 * 1024).cloned();
    let plugins = files.keys().filter_map(|k| k.strip_prefix("plugins/")).filter(|k| k.ends_with(".lsplugin") && !k.contains('/')).map(String::from).collect();
    Ok(Edition { manifest, key, path: path.to_path_buf(), icon, splash, has_pack: files.contains_key("pack.mrpack"), plugins })
}

/// 现在生效的定制包：exe 旁边的优先，再看导入过的
pub fn find(data_dir: &Path) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let beside = std::fs::read_dir(&exe_dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == EXT));
    beside.or_else(|| Some(data_dir.join("edition").join(format!("current.{EXT}"))).filter(|p| p.is_file()))
}

/// 把定制包里带的整合包取出来（装的时候用）
pub fn extract_pack(ed: &Edition, data_dir: &Path) -> Result<PathBuf> {
    let files = sign::read_zip(&ed.path)?;
    sign::check(&files)?;
    let b = files.get("pack.mrpack").context("这个定制包没有带整合包")?;
    let out = data_dir.join("edition").join("pack.mrpack");
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(&out, b)?;
    Ok(out)
}

/// 把定制包带的插件装上（还没装、或者换了版本的）。插件本身的签名照常查；
/// 权限按它申请的给（玩家选了用这个定制包），只有 native 不给（原生插件要认证开发者，走正常安装）。
/// 返回这次新装的插件名
pub fn install_plugins(ed: &Edition, data_dir: &Path) -> Result<Vec<String>> {
    if ed.plugins.is_empty() {
        return Ok(vec![]);
    }
    let files = sign::read_zip(&ed.path)?;
    sign::check(&files)?;
    let tmp = data_dir.join("edition").join("plugins");
    std::fs::create_dir_all(&tmp)?;
    let st = super::host::load_state(data_dir);
    let mut done = vec![];
    for name in &ed.plugins {
        let Some(b) = files.get(&format!("plugins/{name}")) else { continue };
        let p = tmp.join(name);
        std::fs::write(&p, b)?;
        let (m, _, sha, _) = super::host::inspect(&p)?;
        if st.plugins.get(&m.id).is_some_and(|r| r.sha256 == sha) {
            continue;
        }
        let granted: Vec<String> = m.permissions.iter().filter(|x| *x != "native").cloned().collect();
        match super::host::install(data_dir, &p, &granted, "edition", "community") {
            Ok(_) => done.push(m.name.clone()),
            Err(e) => eprintln!("定制包带的插件 {name} 没装上：{e:#}"),
        }
    }
    Ok(done)
}

/// 做一个定制包：清单 + 图标 + 启动画面 + 整合包 + 插件，用作者自己的密钥签名
#[cfg(test)]
pub fn build(manifest: &Value, icon: Option<&Path>, pack: Option<&Path>, out: &Path, pkcs8: &[u8]) -> Result<Vec<u8>> {
    build_with(manifest, icon, None, pack, &[], out, pkcs8)
}

pub fn build_with(manifest: &Value, icon: Option<&Path>, splash: Option<&Path>, pack: Option<&Path>, plugins: &[PathBuf], out: &Path, pkcs8: &[u8]) -> Result<Vec<u8>> {
    let mut m = manifest.clone();
    m["format"] = json!("leafstarter-edition/1");
    let m = clean(&m)?;
    let mut files = BTreeMap::new();
    files.insert("edition.json".to_string(), serde_json::to_vec_pretty(&m)?);
    if let Some(p) = icon {
        let b = std::fs::read(p).with_context(|| format!("读不了图标 {}", p.display()))?;
        if !b.starts_with(b"\x89PNG") || b.len() > 512 * 1024 {
            bail!("图标要是 512 KB 以内的 PNG");
        }
        files.insert("icon.png".into(), b);
    }
    if let Some(p) = splash {
        let b = std::fs::read(p).with_context(|| format!("读不了启动画面 {}", p.display()))?;
        if !b.starts_with(b"\x89PNG") || b.len() > 2 * 1024 * 1024 {
            bail!("启动画面要是 2 MB 以内的 PNG");
        }
        files.insert("splash.png".into(), b);
    }
    if let Some(p) = pack {
        files.insert("pack.mrpack".into(), std::fs::read(p).with_context(|| format!("读不了整合包 {}", p.display()))?);
    }
    for p in plugins.iter().take(10) {
        // 先确认是个签好名的插件包
        super::host::inspect(p).with_context(|| format!("{} 不是能用的插件包", p.display()))?;
        let name = p.file_name().and_then(|n| n.to_str()).context("插件文件名不对")?;
        files.insert(format!("plugins/{name}"), std::fs::read(p)?);
    }
    sign::write_zip(out, &files, pkcs8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_load() {
        let k = sign::new_key().unwrap();
        let d = std::env::temp_dir().join(format!("ls-ed-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let out = d.join("x.lsedition");
        let m = json!({ "name": "星辰服", "author": "管理组", "theme": { "color": "#123456" }, "features": { "download": false }, "servers": [{ "name": "主服", "address": "mc.example.com" }], "update": "http://bad" });
        assert!(build(&m, None, None, &out, &k).is_err(), "http 的更新地址不行");
        let mut m = m;
        m["update"] = json!("");
        build(&m, None, None, &out, &k).unwrap();
        let e = load(&out).unwrap();
        assert_eq!(e.manifest["name"], "星辰服");
        assert_eq!(e.manifest["features"]["download"], false);
        assert_eq!(e.manifest["features"]["ai"], true);
        assert_eq!(e.key, sign::public_of(&k).unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }
}
