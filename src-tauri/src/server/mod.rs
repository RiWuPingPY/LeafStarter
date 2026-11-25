//! 常驻服务器（M10，ROADMAP 5.12 ⑤⑥）：云服务器一键开服、导入已有服务器、面板服。
//! 服务器清单存在 `<数据目录>/servers.json`；密钥、RCON 密码、面板 Key 都用 vault 加密。

pub mod cloud;
pub mod panel;
pub mod playbook;
pub mod rcon;
pub mod ssh;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::auth::vault;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// ssh / mcsm / ptero
    pub kind: String,
    pub host: String,
    pub ssh_port: u16,
    pub user: String,
    pub fingerprint: String,
    /// 加密过的私钥
    pub key: String,
    /// 对应的本地实例（dir/name）
    pub inst: String,
    pub mc: String,
    pub loader: String,
    pub loader_version: String,
    pub java: u32,
    pub mem_mb: u32,
    pub port: u16,
    pub rcon_port: u16,
    /// 加密过的 RCON 密码
    pub rcon_pass: String,
    pub online: bool,
    pub whitelist: Vec<String>,
    pub motd: String,
    /// 剧本里做完的步骤
    pub done: Vec<String>,
    /// 上次失败的步骤和输出
    pub failed: Option<(String, String)>,
    /// 接管的已有服务器：服务端目录（不是我们装的）
    pub adopted_dir: String,
    pub panel_base: String,
    pub panel_key: String,
    pub panel_inst: String,
    pub panel_daemon: String,
    pub created: u64,
}

impl Entry {
    /// 给界面看的（不带任何密钥）
    pub fn view(&self) -> Value {
        json!({
            "id": self.id, "name": self.name, "kind": self.kind, "host": self.host, "sshPort": self.ssh_port, "user": self.user,
            "fingerprint": self.fingerprint, "inst": self.inst, "mc": self.mc, "loader": self.loader, "loaderVersion": self.loader_version,
            "memMb": self.mem_mb, "port": self.port, "online": self.online, "whitelist": self.whitelist, "motd": self.motd,
            "done": self.done, "failed": self.failed, "setup": self.kind == "ssh" && self.done.iter().any(|d| d == "start"),
            "panelBase": self.panel_base, "panelInst": self.panel_inst, "created": self.created,
            "address": if self.port == 25565 || self.port == 0 { self.host.clone() } else { format!("{}:{}", self.host, self.port) },
        })
    }
    pub fn auth(&self) -> Result<ssh::Auth> {
        Ok(ssh::Auth::Key(String::from_utf8(vault::open(&self.key)?)?))
    }
    pub fn rcon_password(&self) -> Result<String> {
        Ok(String::from_utf8(vault::open(&self.rcon_pass)?)?)
    }
    pub fn panel(&self) -> Result<panel::Panel> {
        Ok(panel::Panel { kind: self.kind.clone(), base: self.panel_base.clone(), key: String::from_utf8(vault::open(&self.panel_key)?)? })
    }
    pub async fn connect(&self) -> Result<ssh::Conn> {
        if self.kind != "ssh" {
            bail!("这不是 SSH 服务器");
        }
        let fp = (!self.fingerprint.is_empty()).then_some(self.fingerprint.as_str());
        ssh::connect(&self.host, self.ssh_port, &self.user, &self.auth()?, fp).await
    }
}

static LOCK: Mutex<()> = Mutex::new(());

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("servers.json")
}

pub fn load(data_dir: &Path) -> Vec<Entry> {
    std::fs::read(path(data_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn get(data_dir: &Path, id: &str) -> Result<Entry> {
    load(data_dir).into_iter().find(|e| e.id == id).context("找不到这台服务器")
}

/// 改一台（不存在就加上）
pub fn put(data_dir: &Path, e: &Entry) -> Result<()> {
    let _g = LOCK.lock().unwrap();
    let mut all = load(data_dir);
    match all.iter_mut().find(|x| x.id == e.id) {
        Some(x) => *x = e.clone(),
        None => all.push(e.clone()),
    }
    std::fs::write(path(data_dir), serde_json::to_vec_pretty(&all)?)?;
    Ok(())
}

pub fn update(data_dir: &Path, id: &str, f: impl FnOnce(&mut Entry)) -> Result<Entry> {
    let _g = LOCK.lock().unwrap();
    let mut all = load(data_dir);
    let e = all.iter_mut().find(|x| x.id == id).context("找不到这台服务器")?;
    f(e);
    let out = e.clone();
    std::fs::write(path(data_dir), serde_json::to_vec_pretty(&all)?)?;
    Ok(out)
}

pub fn remove(data_dir: &Path, id: &str) -> Result<()> {
    let _g = LOCK.lock().unwrap();
    let mut all = load(data_dir);
    all.retain(|x| x.id != id);
    std::fs::write(path(data_dir), serde_json::to_vec_pretty(&all)?)?;
    Ok(())
}

/// 随机字母数字（RCON 密码、服务器 id）
pub fn token(n: usize) -> String {
    const A: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    crate::sync::crypto::random::<64>().iter().take(n).map(|b| A[*b as usize % A.len()] as char).collect()
}

// ---------- 首次连接：密码换密钥 ----------

/// 用密码（或用户自己的私钥）登录，装上我们的公钥，再用新密钥登录一次确认能用。返回 (加密的私钥, 指纹)
pub async fn enroll(host: &str, port: u16, user: &str, auth: ssh::Auth) -> Result<(String, String)> {
    let c = ssh::connect(host, port, user, &auth, None).await?;
    let fp = c.fingerprint.clone();
    let (private, public) = ssh::gen_key(&format!("leafstarter@{}", whoami()))?;
    let cmd = format!(
        "umask 077; mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && (grep -qxF {k} ~/.ssh/authorized_keys || echo {k} >> ~/.ssh/authorized_keys) && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys",
        k = ssh::shq(&public)
    );
    c.run(&cmd).await.context("装公钥失败")?;
    c.close().await;
    // 用新钥匙再登一次，确认能用（指纹也要对得上）
    let c2 = ssh::connect(host, port, user, &ssh::Auth::Key(private.clone()), Some(&fp)).await.context("公钥装上了但用它登录不了（服务器可能关了密钥登录）")?;
    c2.close().await;
    Ok((vault::seal(private.as_bytes())?, fp))
}

fn whoami() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "pc".into()).to_lowercase()
}

/// 关掉密码登录（先确认密钥能登录才做，免得把自己锁在外面）
pub const DISABLE_PASSWORD: &str = r#"SUDO=""; [ "$(id -u)" = 0 ] || SUDO="sudo -n"
for f in /etc/ssh/sshd_config /etc/ssh/sshd_config.d/*.conf; do
  [ -f "$f" ] || continue
  $SUDO sed -i -E 's/^[#[:space:]]*PasswordAuthentication[[:space:]].*/PasswordAuthentication no/' "$f"
done
grep -qE '^PasswordAuthentication no' /etc/ssh/sshd_config || echo 'PasswordAuthentication no' | $SUDO tee -a /etc/ssh/sshd_config >/dev/null
$SUDO systemctl reload ssh 2>/dev/null || $SUDO systemctl reload sshd 2>/dev/null || $SUDO service ssh reload 2>/dev/null || true
echo "关掉了密码登录，以后只能用密钥"
"#;

// ---------- 剧本参数 ----------

/// 按本地实例算出剧本参数：服务端下载地址、Java 版本
pub async fn make_plan(net: &crate::base::net::Net, e: &Entry) -> Result<playbook::Plan> {
    let manifest = crate::game::mojang::fetch_manifest(net).await?;
    let mv = manifest.versions.iter().find(|v| v.id == e.mc).with_context(|| format!("版本清单里没有 {}", e.mc))?;
    let vj = crate::game::mojang::fetch_version_json(net, mv).await?;
    let srv = &vj["downloads"]["server"];
    let (url, sha1) = (srv["url"].as_str().context("这个版本没有官方服务端")?, srv["sha1"].as_str().unwrap_or(""));
    let java = vj["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32;
    let bm = crate::base::net::BMCLAPI_BASE;
    let (mc, lv_full) = (e.mc.as_str(), e.loader_version.as_str());
    // 老 Forge 的版本号带「-1.7.10」这样的后缀：BMCLAPI 要去掉，官方 maven 路径要带着
    let suffix = format!("-{mc}");
    let lv = lv_full.strip_suffix(suffix.as_str()).unwrap_or(lv_full);
    let extra = match e.loader.as_str() {
        "fabric" => {
            let inst = fabric_installer().await.unwrap_or_else(|_| "1.0.1".into());
            Some((format!("{bm}/fabric-meta/v2/versions/loader/{mc}/{lv}/{inst}/server/jar"), format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{lv}/{inst}/server/jar")))
        }
        "forge" => Some((
            format!("{bm}/forge/download?mcversion={mc}&version={lv}&category=installer&format=jar"),
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{lv_full}/forge-{mc}-{lv_full}-installer.jar"),
        )),
        "neoforge" => Some((
            format!("{bm}/maven/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar"),
            format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar"),
        )),
        "vanilla" | "" => None,
        other => bail!("{other} 的服务端还不支持（支持原版、Fabric、Forge、NeoForge）"),
    };
    Ok(playbook::Plan {
        id: e.id.clone(),
        name: e.name.clone(),
        mc: e.mc.clone(),
        loader: if e.loader.is_empty() { "vanilla".into() } else { e.loader.clone() },
        loader_version: e.loader_version.clone(),
        java,
        mem_mb: if e.mem_mb == 0 { 2048 } else { e.mem_mb },
        port: if e.port == 0 { 25565 } else { e.port },
        rcon_port: e.rcon_port,
        rcon_pass: e.rcon_password()?,
        online: e.online,
        whitelist: !e.whitelist.is_empty(),
        motd: if e.motd.is_empty() { format!("{}（LeafStarter）", e.name) } else { e.motd.clone() },
        server_jar: Some((format!("{bm}/version/{mc}/server"), url.to_string(), sha1.to_string())),
        extra_jar: extra,
        keep_backups: 7,
    })
}

async fn fabric_installer() -> Result<String> {
    let v: Value = reqwest::Client::new().get("https://meta.fabricmc.net/v2/versions/installer").timeout(std::time::Duration::from_secs(10)).send().await?.json().await?;
    v.as_array().and_then(|a| a.iter().find(|x| x["stable"].as_bool() == Some(true)).or(a.first())).and_then(|x| x["version"].as_str()).map(String::from).context("没拿到 Fabric 安装器版本")
}

// ---------- 同步 Mod 和配置 ----------

/// 本地实例里要放到服务器上的 Mod：(文件名, 路径, sha1)。只在客户端用的（Modrinth 上 server_side = unsupported）剔掉
pub async fn server_mods(mods_dir: &Path, log: &(dyn Fn(String) + Sync)) -> Result<Vec<(String, PathBuf, String)>> {
    let mut list = vec![];
    for e in std::fs::read_dir(mods_dir).into_iter().flatten().flatten() {
        let p = e.path();
        let n = e.file_name().to_string_lossy().into_owned();
        if p.is_file() && n.to_lowercase().ends_with(".jar") {
            let h = crate::base::net::sha1_file(&p)?;
            list.push((n, p, h));
        }
    }
    if list.is_empty() {
        return Ok(list);
    }
    let http = crate::content::modrinth::client()?;
    let hashes: Vec<String> = list.iter().map(|x| x.2.clone()).collect();
    let found = crate::content::modrinth::by_hashes(&http, &hashes).await.unwrap_or_default();
    let ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
    let projects = crate::content::modrinth::projects(&http, &ids).await.unwrap_or_default();
    let mut keep = vec![];
    for x in list {
        let side = found.get(&x.2).and_then(|v| projects.get(&v.project_id)).and_then(|p| p["serverSide"].as_str()).unwrap_or("");
        if side == "unsupported" {
            log(format!("跳过只在客户端用的：{}", x.0));
        } else {
            keep.push(x);
        }
    }
    Ok(keep)
}

/// 把 Mod 和配置同步到服务器：多的删掉（只管 mods 里的 .jar）、少的传上去、变了的重传；config 打包传上去覆盖
pub async fn sync_mods(c: &ssh::Conn, e: &Entry, game_dir: &Path, log: &(dyn Fn(String) + Sync)) -> Result<String> {
    let base = playbook::base(&e.id);
    let sdir = if e.adopted_dir.is_empty() { format!("{base}/server") } else { e.adopted_dir.clone() };
    let mods = server_mods(&game_dir.join("mods"), log).await?;
    let remote = c.run(&format!("mkdir -p \"{sdir}/mods\" && cd \"{sdir}/mods\" && (sha1sum *.jar 2>/dev/null || true)")).await?;
    let remote: HashMap<String, String> = remote.lines().filter_map(|l| l.split_once("  ").map(|(h, n)| (n.trim().to_string(), h.to_string()))).collect();
    let want: HashSet<&str> = mods.iter().map(|m| m.0.as_str()).collect();
    let extra: Vec<&String> = remote.keys().filter(|n| !want.contains(n.as_str())).collect();
    if !extra.is_empty() {
        log(format!("删掉服务器上多出来的 {} 个 Mod", extra.len()));
        let names = extra.iter().map(|n| ssh::shq(n)).collect::<Vec<_>>().join(" ");
        c.run(&format!("cd \"{sdir}/mods\" && rm -f -- {names}")).await?;
    }
    let mut sent = 0;
    for (n, p, h) in &mods {
        if remote.get(n) == Some(h) {
            continue;
        }
        log(format!("传 {n}"));
        let f = tokio::fs::File::open(p).await?;
        c.upload(f, &format!("{sdir}/mods/{n}")).await.with_context(|| format!("传 {n} 失败"))?;
        sent += 1;
    }
    // 配置：config/ 和 defaultconfigs/ 打包传上去
    let mut cfgs = 0;
    for d in ["config", "defaultconfigs"] {
        let src = game_dir.join(d);
        if !src.is_dir() {
            continue;
        }
        let tmp = std::env::temp_dir().join(format!("ls-{}-{d}.tar.gz", e.id));
        let (src2, tmp2) = (src.clone(), tmp.clone());
        tokio::task::spawn_blocking(move || pack_dir(&src2, &tmp2, None)).await??;
        let f = tokio::fs::File::open(&tmp).await?;
        let up = format!("{base}/tmp/{d}.tar.gz");
        c.upload(f, &up).await?;
        c.run(&format!("mkdir -p \"{sdir}/{d}\" && tar xzf \"{up}\" -C \"{sdir}/{d}\" && rm -f \"{up}\"")).await?;
        let _ = std::fs::remove_file(&tmp);
        cfgs += 1;
    }
    Ok(format!("服务器上 {} 个 Mod（新传 {sent} 个、删掉 {} 个）{}", mods.len(), extra.len(), if cfgs > 0 { "，配置也同步了" } else { "" }))
}

/// 把一个文件夹打成 tar.gz（`prefix` 是包里的顶层目录名，None 表示不加）
pub fn pack_dir(src: &Path, out: &Path, prefix: Option<&str>) -> Result<()> {
    let f = std::fs::File::create(out)?;
    let gz = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
    let mut t = tar::Builder::new(gz);
    t.follow_symlinks(false);
    add_dir(&mut t, src, Path::new(prefix.unwrap_or(".")))?;
    t.into_inner()?.finish()?;
    Ok(())
}

/// 一层层加进去；session.lock（游戏开着时被锁住）跳过
fn add_dir<W: std::io::Write>(t: &mut tar::Builder<W>, dir: &Path, name: &Path) -> Result<()> {
    t.append_dir(name, dir)?;
    for e in std::fs::read_dir(dir)?.flatten() {
        let p = e.path();
        let n = name.join(e.file_name());
        let ft = e.file_type()?;
        if ft.is_dir() {
            add_dir(t, &p, &n)?;
        } else if ft.is_file() && e.file_name() != "session.lock" {
            t.append_path_with_name(&p, &n).with_context(|| format!("读不了 {}", p.display()))?;
        }
    }
    Ok(())
}

/// 解开 tar.gz 到一个文件夹（只认普通文件和目录，路径不许跳出去）
pub fn unpack(archive: &Path, dest: &Path, strip_first: bool) -> Result<u64> {
    let f = std::fs::File::open(archive)?;
    let mut t = tar::Archive::new(flate2::read::GzDecoder::new(f));
    let mut n = 0;
    for entry in t.entries()? {
        let mut entry = entry?;
        let p = entry.path()?.into_owned();
        let mut comps: Vec<String> = p.components().filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        }).collect();
        if strip_first && !comps.is_empty() {
            comps.remove(0);
        }
        if comps.is_empty() || p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            continue;
        }
        let target = comps.iter().fold(dest.to_path_buf(), |acc, c| acc.join(c));
        match entry.header().entry_type() {
            tar::EntryType::Directory => std::fs::create_dir_all(&target)?,
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                if let Some(d) = target.parent() {
                    std::fs::create_dir_all(d)?;
                }
                entry.unpack(&target)?;
                n += 1;
            }
            _ => {}
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pack_roundtrip() {
        let d = std::env::temp_dir().join(format!("ls-pack-{}", std::process::id()));
        let src = d.join("src");
        std::fs::create_dir_all(src.join("region")).unwrap();
        std::fs::write(src.join("level.dat"), b"abc").unwrap();
        std::fs::write(src.join("region").join("r.0.0.mca"), b"xyz").unwrap();
        let out = d.join("w.tar.gz");
        super::pack_dir(&src, &out, Some("world")).unwrap();
        let dst = d.join("dst");
        assert_eq!(super::unpack(&out, &dst, true).unwrap(), 2);
        assert_eq!(std::fs::read(dst.join("region").join("r.0.0.mca")).unwrap(), b"xyz");
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(super::token(12).len(), 12);
    }
}
