//! 插件宿主（M15–M17，ROADMAP 5.18）。四层：
//! ① 外观（主题变量、字体）② 声明式（崩溃规则、镜像源、命令面板条目、主页卡片、外置登录预设）
//! ③ 脚本（单文件 HTML，跑在 `sandbox="allow-scripts"` 的 iframe 里，只能通过宿主按权限办事）
//! ④ 原生（带独立程序，JSON-RPC 走标准输入输出；只收认证开发者的）
//!
//! 包是签名的 zip（`.lsplugin`，签名同定制包）。装在 `<数据目录>/plugins/<id>/`，状态在 `plugins/state.json`。
//! 防坏插件：作者签名、市场索引里的 SHA-256、吊销名单、更新后权限变多要重新同意、安全模式（按住 Ctrl 打开不加载插件）。

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::sign;

pub const EXT: &str = "lsplugin";
/// 插件市场索引：放在主仓库的 `market/` 里（插件文件也在那儿，或者作者自己的 https 地址），按 SHA-256 校验
pub const INDEX_URL: &str = "https://raw.githubusercontent.com/RiWuPingPY/LeafStarter/main/market/index.json";

/// 认证开发者的根公钥（base64）。原生插件的 `cert` 要由其中一把签过作者公钥才收。
/// 根私钥不进仓库；发证书用 `leafstarter --cli plugin-cert <根私钥文件> <作者公钥>`
pub const ROOT_KEYS: &[&str] = &["O04AqH7RLpUGCcSN86gjhMKiod/ezLPqaK4q48DkUfs="];

/// 能申请的权限和说明（安装时给用户看）
pub fn perm_text(p: &str) -> Option<String> {
    Some(match p {
        "instances:read" => "读你的实例列表和 Mod 列表".into(),
        "launch" => "启动游戏（每次都会问你）".into(),
        "events" => "知道游戏什么时候启动、退出、崩溃".into(),
        "storage" => "在本机存一点自己的数据".into(),
        "native" => "运行插件自带的程序（只有认证开发者的插件能要）".into(),
        _ if p.starts_with("net:") && p.len() > 4 => format!("访问 {}", &p[4..]),
        _ => return None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    /// look / decl / script / native
    pub layer: String,
    pub permissions: Vec<String>,
    pub contributes: Value,
    pub main: String,
    /// 平台 → 程序路径（原生插件）
    pub bin: BTreeMap<String, String>,
    /// 原生插件：根密钥对作者公钥的签名（base64）
    pub cert: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Record {
    pub enabled: bool,
    pub version: String,
    /// 作者公钥指纹（本地开发的是空的）
    pub key: String,
    /// official / reviewed / community / dev
    pub tier: String,
    pub source: String,
    pub sha256: String,
    pub granted: Vec<String>,
    pub installed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub plugins: BTreeMap<String, Record>,
    /// 吊销的：id → 原因
    pub revoked: BTreeMap<String, String>,
    /// 开发者模式：本地插件文件夹
    pub dev_dir: String,
    pub dev: bool,
}

static LOCK: Mutex<()> = Mutex::new(());

pub fn root(data_dir: &Path) -> PathBuf {
    data_dir.join("plugins")
}

pub fn load_state(data_dir: &Path) -> State {
    std::fs::read(root(data_dir).join("state.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_state(data_dir: &Path, s: &State) -> Result<()> {
    std::fs::create_dir_all(root(data_dir))?;
    std::fs::write(root(data_dir).join("state.json"), serde_json::to_vec_pretty(s)?)?;
    Ok(())
}

pub fn update_state<T>(data_dir: &Path, f: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
    let _g = LOCK.lock().unwrap();
    let mut s = load_state(data_dir);
    let r = f(&mut s)?;
    save_state(data_dir, &s)?;
    Ok(r)
}

/// 清单检查
pub fn parse_manifest(b: &[u8]) -> Result<Manifest> {
    let m: Manifest = serde_json::from_slice(b).context("plugin.json 坏了")?;
    if !(2..=40).contains(&m.id.len()) || !m.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        bail!("插件 id 只能用小写字母、数字和连字符，2 到 40 个字");
    }
    if m.name.trim().is_empty() || m.version.trim().is_empty() {
        bail!("插件要有名字和版本");
    }
    if !["look", "decl", "script", "native"].contains(&m.layer.as_str()) {
        bail!("不认识的插件层级：{}", m.layer);
    }
    for p in &m.permissions {
        if perm_text(p).is_none() {
            bail!("不认识的权限：{p}");
        }
    }
    if matches!(m.layer.as_str(), "script" | "native") && m.main.is_empty() {
        bail!("脚本插件要写 main（入口 HTML）");
    }
    if m.layer == "native" && !m.permissions.iter().any(|p| p == "native") {
        bail!("原生插件要申请 native 权限");
    }
    Ok(m)
}

/// 原生插件的证书：某把根密钥对作者公钥的签名
pub fn cert_ok(m: &Manifest, author_key: &[u8]) -> bool {
    let Some(sig) = sign::unb64(&m.cert) else { return false };
    let mut roots: Vec<Vec<u8>> = ROOT_KEYS.iter().filter_map(|k| sign::unb64(k)).collect();
    // 调试版可以用环境变量加一把测试根密钥
    if cfg!(debug_assertions) {
        if let Some(k) = std::env::var("LEAFSTARTER_DEV_ROOT").ok().and_then(|k| sign::unb64(&k)) {
            roots.push(k);
        }
    }
    roots.iter().any(|r| sign::verify(r, author_key, &sig))
}

/// 看一个插件包：校验签名，返回清单、作者指纹、包的 SHA-256
pub fn inspect(path: &Path) -> Result<(Manifest, Vec<u8>, String, BTreeMap<String, Vec<u8>>)> {
    let raw = std::fs::read(path).with_context(|| format!("读不了 {}", path.display()))?;
    let sha = { use sha2::Digest; format!("{:x}", sha2::Sha256::digest(&raw)) };
    let files = sign::read_zip(path)?;
    let key = sign::check(&files)?;
    let m = parse_manifest(files.get("plugin.json").context("包里没有 plugin.json")?)?;
    if m.layer == "native" && !cert_ok(&m, &key) {
        bail!("原生插件只收认证开发者的；这个插件的作者没有有效的开发者证书");
    }
    if matches!(m.layer.as_str(), "script" | "native") && !files.contains_key(&m.main) {
        bail!("包里没有入口文件 {}", m.main);
    }
    Ok((m, key, sha, files))
}

/// 装（或升级）：解到 plugins/<id>/，原来的整个换掉
pub fn install(data_dir: &Path, path: &Path, granted: &[String], source: &str, tier: &str) -> Result<Manifest> {
    let (m, key, sha, files) = inspect(path)?;
    let st = load_state(data_dir);
    if let Some(r) = st.revoked.get(&m.id) {
        bail!("这个插件被吊销了：{r}");
    }
    // 要的权限都得同意过
    let missing: Vec<&String> = m.permissions.iter().filter(|p| !granted.contains(p)).collect();
    if !missing.is_empty() {
        bail!("还没同意这些权限：{}", missing.iter().map(|p| p.as_str()).collect::<Vec<_>>().join("、"));
    }
    // 升级时作者密钥要和原来一样（防止别人冒名推更新）
    if let Some(old) = st.plugins.get(&m.id) {
        if !old.key.is_empty() && old.key != sign::fingerprint(&key) {
            bail!("新包不是原作者签的（签名指纹变了），没有装");
        }
    }
    // 直接解到最终目录（有的电脑上 AppData 里改名会失败，见 util::move_file）
    let dir = root(data_dir).join(&m.id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).context("删不掉旧版本（插件程序可能还在运行）")?;
    }
    for (name, data) in &files {
        let p = name.split('/').fold(dir.clone(), |acc, c| acc.join(c));
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, data)?;
    }
    update_state(data_dir, |s| {
        s.plugins.insert(m.id.clone(), Record { enabled: true, version: m.version.clone(), key: sign::fingerprint(&key), tier: tier.into(), source: source.into(), sha256: sha.clone(), granted: m.permissions.clone(), installed: crate::base::util::now_ms() });
        Ok(())
    })?;
    Ok(m)
}

pub fn remove(data_dir: &Path, id: &str) -> Result<()> {
    if id.contains(['/', '\\', '.']) {
        bail!("id 不对");
    }
    let _ = std::fs::remove_dir_all(root(data_dir).join(id));
    update_state(data_dir, |s| {
        s.plugins.remove(id);
        Ok(())
    })
}

/// 读装好的插件的清单（开发者模式的从文件夹读）
pub fn manifest_of(data_dir: &Path, id: &str, st: &State) -> Option<(Manifest, PathBuf)> {
    if id == "dev" || st.plugins.get(id).is_some_and(|r| r.tier == "dev") {
        let d = PathBuf::from(&st.dev_dir);
        let m = parse_manifest(&std::fs::read(d.join("plugin.json")).ok()?).ok()?;
        return Some((m, d));
    }
    let d = root(data_dir).join(id);
    let m = parse_manifest(&std::fs::read(d.join("plugin.json")).ok()?).ok()?;
    Some((m, d))
}

/// 列表：装好的 + 开发者模式的本地插件
pub fn list(data_dir: &Path, safe: bool) -> Vec<Value> {
    let st = load_state(data_dir);
    let mut out = vec![];
    for (id, r) in &st.plugins {
        let Some((m, _)) = manifest_of(data_dir, id, &st) else { continue };
        out.push(json!({ "id": id, "manifest": m, "record": r, "revoked": st.revoked.get(id), "active": active(&st, id, safe), "perms": m.permissions.iter().map(|p| json!({ "id": p, "text": perm_text(p) })).collect::<Vec<_>>() }));
    }
    if st.dev && !st.dev_dir.is_empty() {
        match parse_manifest(&std::fs::read(Path::new(&st.dev_dir).join("plugin.json")).unwrap_or_default()) {
            Ok(m) => out.push(json!({ "id": m.id, "dev": true, "manifest": m, "record": { "enabled": true, "tier": "dev" }, "active": !safe, "perms": m.permissions.iter().map(|p| json!({ "id": p, "text": perm_text(p) })).collect::<Vec<_>>() })),
            Err(e) => out.push(json!({ "id": "dev", "dev": true, "error": format!("{e:#}") })),
        }
    }
    out
}

pub fn active(st: &State, id: &str, safe: bool) -> bool {
    !safe && st.plugins.get(id).is_some_and(|r| r.enabled) && !st.revoked.contains_key(id)
}

/// 生效的插件：(清单, 目录, 是不是本地开发的)
pub fn enabled(data_dir: &Path, safe: bool) -> Vec<(Manifest, PathBuf, bool)> {
    if safe {
        return vec![];
    }
    let st = load_state(data_dir);
    let mut out: Vec<(Manifest, PathBuf, bool)> = st.plugins.keys().filter(|id| active(&st, id, false)).filter_map(|id| manifest_of(data_dir, id, &st).map(|(m, d)| (m, d, false))).collect();
    if st.dev && !st.dev_dir.is_empty() {
        let d = PathBuf::from(&st.dev_dir);
        if let Ok(m) = parse_manifest(&std::fs::read(d.join("plugin.json")).unwrap_or_default()) {
            out.retain(|(x, _, _)| x.id != m.id);
            out.push((m, d, true));
        }
    }
    out
}

/// 所有生效插件的贡献，合在一起给界面
pub fn contributions(data_dir: &Path, safe: bool) -> Value {
    let mut themes = vec![];
    let mut rules = vec![];
    let mut mirrors = vec![];
    let mut palette = vec![];
    let mut cards = vec![];
    let mut ygg = vec![];
    let mut scripts = vec![];
    for (m, _, dev) in enabled(data_dir, safe) {
        let c = &m.contributes;
        let tag = |v: &Value| {
            let mut v = v.clone();
            v["plugin"] = json!(m.id);
            v["pluginName"] = json!(m.name);
            v
        };
        if c["theme"].is_object() {
            themes.push(tag(&c["theme"]));
        }
        for (k, out) in [("crashRules", &mut rules), ("mirrors", &mut mirrors), ("palette", &mut palette), ("homeCards", &mut cards), ("yggPresets", &mut ygg)] {
            for x in c[k].as_array().cloned().unwrap_or_default().iter().take(50) {
                out.push(tag(x));
            }
        }
        if matches!(m.layer.as_str(), "script" | "native") {
            scripts.push(json!({ "plugin": m.id, "name": m.name, "card": c["card"], "page": c["page"], "permissions": m.permissions, "dev": dev, "native": m.layer == "native" }));
        }
    }
    json!({ "themes": themes, "crashRules": rules, "mirrors": mirrors, "palette": palette, "homeCards": cards, "yggPresets": ygg, "scripts": scripts })
}

/// 插件的崩溃规则：每条规则的 match 里的字都出现在日志或崩溃报告里才算
pub fn crash_rules(data_dir: &Path, safe: bool, text: &str) -> Option<crate::launch::crash::Diagnosis> {
    for (m, _, _) in enabled(data_dir, safe) {
        for r in m.contributes["crashRules"].as_array().cloned().unwrap_or_default() {
            let words: Vec<&str> = r["match"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).filter(|x| x.len() >= 4).collect()).unwrap_or_default();
            if words.is_empty() || !words.iter().all(|w| text.contains(w)) {
                continue;
            }
            let title: String = r["title"].as_str().unwrap_or("").chars().take(80).collect();
            let detail: String = r["detail"].as_str().unwrap_or("").chars().take(600).collect();
            if title.is_empty() {
                continue;
            }
            return Some(crate::launch::crash::Diagnosis { title, detail: format!("{detail}\n（这条规则来自插件「{}」）", m.name), fix: None, fix_label: String::new() });
        }
    }
    None
}

/// 读插件里的一个文件（脚本插件的入口 HTML）
pub fn read_file(data_dir: &Path, id: &str, name: &str) -> Result<String> {
    if name.split(['/', '\\']).any(|c| c == ".." || c.is_empty()) {
        bail!("路径不对");
    }
    let st = load_state(data_dir);
    let (_, d) = manifest_of(data_dir, id, &st).or_else(|| {
        // 开发者模式的插件可能还没记进 state
        let d = PathBuf::from(&st.dev_dir);
        let m = parse_manifest(&std::fs::read(d.join("plugin.json")).ok()?).ok()?;
        (m.id == id).then_some((m, d))
    }).context("没有这个插件")?;
    let p = name.split('/').fold(d, |acc, c| acc.join(c));
    let b = std::fs::read(&p).with_context(|| format!("读不了 {name}"))?;
    if b.len() > 2 * 1024 * 1024 {
        bail!("入口文件太大了");
    }
    Ok(String::from_utf8_lossy(&b).into_owned())
}

// ---------- 市场索引 ----------

/// 拉市场索引；顺便按吊销名单停用
pub async fn fetch_index(data_dir: &Path, url: &str) -> Result<Value> {
    let v: Value = reqwest::Client::builder()
        .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(20))
        .build()?
        .get(url)
        .send()
        .await
        .context("连不上插件市场")?
        .error_for_status()
        .context("插件市场返回了错误")?
        .json()
        .await
        .context("插件市场的索引坏了")?;
    let revoked: BTreeMap<String, String> = v["revoked"].as_array().map(|a| a.iter().filter_map(|r| Some((r["id"].as_str()?.to_string(), r["reason"].as_str().unwrap_or("插件市场下架了它").to_string()))).collect()).unwrap_or_default();
    update_state(data_dir, |s| {
        for (id, why) in &revoked {
            s.revoked.insert(id.clone(), why.clone());
            if let Some(r) = s.plugins.get_mut(id) {
                r.enabled = false;
            }
        }
        Ok(())
    })?;
    Ok(v)
}

/// 从市场装：下载、对 SHA-256、再按普通包装
pub async fn download(data_dir: &Path, url: &str, sha256: &str) -> Result<PathBuf> {
    if !url.starts_with("https://") {
        bail!("下载地址要用 https");
    }
    let b = reqwest::Client::builder().user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION"))).timeout(std::time::Duration::from_secs(120)).build()?.get(url).send().await?.error_for_status()?.bytes().await?;
    let got = { use sha2::Digest; format!("{:x}", sha2::Sha256::digest(&b)) };
    if !got.eq_ignore_ascii_case(sha256.trim()) {
        bail!("下载的文件和市场登记的哈希对不上，没有装");
    }
    let p = root(data_dir).join("downloads").join(format!("{got}.{EXT}"));
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, &b)?;
    Ok(p)
}

// ---------- 原生插件 ----------

/// 跑着的原生插件进程：id → (标准输入, 标准输出)
pub struct Native {
    pub procs: tokio::sync::Mutex<HashMap<String, NativeProc>>,
}

pub struct NativeProc {
    child: tokio::process::Child,
    /// 作业对象：结束时连它起的子进程一起结束（比如 .cmd 里起的 node）
    #[cfg(windows)]
    job: Option<crate::base::gpu::Job>,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
    seq: u64,
}

impl Default for Native {
    fn default() -> Self {
        Native { procs: tokio::sync::Mutex::new(HashMap::new()) }
    }
}

fn platform() -> &'static str {
    if cfg!(all(windows, target_arch = "x86_64")) {
        "windows-x64"
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        "windows-arm64"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux-x64"
    }
}

impl Native {
    /// 调一次原生插件：一行一条 JSON-RPC
    pub async fn call(&self, data_dir: &Path, safe: bool, id: &str, method: &str, params: Value) -> Result<Value> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let (m, dir, _) = enabled(data_dir, safe).into_iter().find(|(m, _, _)| m.id == id).context("这个插件没开着")?;
        if m.layer != "native" {
            bail!("这不是原生插件");
        }
        let mut procs = self.procs.lock().await;
        let alive = match procs.get_mut(id) {
            Some(p) => p.child.try_wait().ok().flatten().is_none(),
            None => false,
        };
        if !alive {
            let rel = m.bin.get(platform()).context("这个插件没有这个平台的程序")?;
            if rel.split(['/', '\\']).any(|c| c == "..") {
                bail!("程序路径不对");
            }
            let exe = rel.split('/').fold(dir.clone(), |acc, c| acc.join(c));
            let mut cmd = tokio::process::Command::new(&exe);
            cmd.current_dir(&dir).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
            #[cfg(windows)]
            cmd.creation_flags(0x0800_0000);
            let mut child = cmd.spawn().with_context(|| format!("启动不了 {}", exe.display()))?;
            #[cfg(windows)]
            let job = child.raw_handle().and_then(crate::base::gpu::Job::adopt);
            let stdin = child.stdin.take().context("没有 stdin")?;
            let stdout = tokio::io::BufReader::new(child.stdout.take().context("没有 stdout")?).lines();
            procs.insert(id.to_string(), NativeProc {
                child,
                #[cfg(windows)]
                job,
                stdin,
                stdout,
                seq: 0,
            });
        }
        let p = procs.get_mut(id).unwrap();
        p.seq += 1;
        let rid = p.seq;
        p.stdin.write_all(format!("{}\n", json!({ "jsonrpc": "2.0", "id": rid, "method": method, "params": params })).as_bytes()).await?;
        p.stdin.flush().await?;
        let line = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                match p.stdout.next_line().await? {
                    Some(l) => {
                        let v: Value = serde_json::from_str(&l).unwrap_or(Value::Null);
                        if v["id"].as_u64() == Some(rid) {
                            return anyhow::Ok(v);
                        }
                    }
                    None => bail!("插件程序退出了"),
                }
            }
        })
        .await
        .context("插件程序 30 秒没回应")??;
        if let Some(e) = line.get("error") {
            bail!("插件程序报错：{}", e["message"].as_str().unwrap_or(&e.to_string()));
        }
        Ok(line["result"].clone())
    }

    pub async fn stop(&self, id: &str) {
        if let Some(mut p) = self.procs.lock().await.remove(id) {
            #[cfg(windows)]
            if let Some(j) = &p.job {
                j.kill_all();
            }
            let _ = p.child.kill().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(dir: &Path, manifest: Value, extra: &[(&str, &[u8])], key: &[u8]) -> PathBuf {
        let mut files = BTreeMap::new();
        files.insert("plugin.json".to_string(), serde_json::to_vec(&manifest).unwrap());
        for (n, b) in extra {
            files.insert(n.to_string(), b.to_vec());
        }
        let p = dir.join(format!("{}.lsplugin", manifest["id"].as_str().unwrap()));
        sign::write_zip(&p, &files, key).unwrap();
        p
    }

    #[test]
    fn install_flow() {
        let d = std::env::temp_dir().join(format!("ls-plug-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let k = sign::new_key().unwrap();
        let theme = json!({ "id": "red-theme", "name": "红色主题", "version": "1.0.0", "layer": "look", "contributes": { "theme": { "light": { "--accent": "#C0392B" } } } });
        let p = pack(&d, theme.clone(), &[], &k);
        install(&d, &p, &[], "file", "community").unwrap();
        assert_eq!(contributions(&d, false)["themes"].as_array().unwrap().len(), 1);
        assert_eq!(contributions(&d, true)["themes"].as_array().unwrap().len(), 0, "安全模式不加载");
        // 权限：没同意不装
        let rule = json!({ "id": "rules", "name": "规则包", "version": "1.0.0", "layer": "decl", "permissions": ["events"], "contributes": { "crashRules": [{ "title": "测试规则", "detail": "x", "match": ["FooBarException", "baz.mixins"] }] } });
        let p2 = pack(&d, rule, &[], &k);
        assert!(install(&d, &p2, &[], "file", "community").is_err());
        install(&d, &p2, &["events".into()], "file", "community").unwrap();
        assert!(crash_rules(&d, false, "at FooBarException in baz.mixins.json").is_some());
        assert!(crash_rules(&d, false, "FooBarException only").is_none());
        // 换了作者的密钥不让升级
        let k2 = sign::new_key().unwrap();
        let mut t2 = theme.clone();
        t2["version"] = json!("1.0.1");
        let p3 = pack(&d, t2, &[], &k2);
        assert!(install(&d, &p3, &[], "file", "community").is_err());
        // 原生插件没证书不收
        let nat = json!({ "id": "nat", "name": "原生", "version": "1.0.0", "layer": "native", "permissions": ["native"], "main": "index.html", "bin": { "windows-x64": "bin/x.exe" } });
        let p4 = pack(&d, nat, &[("index.html", b"<p>hi</p>")], &k);
        assert!(inspect(&p4).unwrap_err().to_string().contains("认证"));
        // 吊销
        update_state(&d, |s| {
            s.revoked.insert("red-theme".into(), "测试".into());
            Ok(())
        })
        .unwrap();
        assert_eq!(contributions(&d, false)["themes"].as_array().unwrap().len(), 0);
        assert!(parse_manifest(br#"{"id":"Bad Id","name":"x","version":"1","layer":"look"}"#).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
