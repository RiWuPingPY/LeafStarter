//! 下载源管理：URL 改写、测速打分、按文件回退。
//!
//! 每个文件按当前得分从高到低依次尝试各个源；某个源连续失败或被限流（429）会冷却一会儿，
//! 实际下载的速度会持续回写到得分里，所以"哪个快用哪个"是在下载过程中动态调整的。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

pub const BMCLAPI_BASE: &str = "https://bmclapi2.bangbang93.com";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// 官方源：Mojang / Fabric / Quilt / Forge / NeoForge 各自的服务器
    Official,
    /// BMCLAPI 及与它路径规则兼容的镜像
    Bmclapi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDef {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    pub base: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetSettings {
    /// "auto"：按实测速度挑；"fixed"：优先用 preferred，失败再回退
    pub mode: String,
    pub preferred: String,
    pub threads: usize,
    #[serde(default)]
    pub custom: Vec<SourceDef>,
}

impl Default for NetSettings {
    fn default() -> Self {
        NetSettings { mode: "auto".into(), preferred: "bmclapi".into(), threads: 24, custom: vec![] }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStat {
    pub latency_ms: Option<u64>,
    /// 测速得到的吞吐（字节/秒）
    pub test_speed: Option<f64>,
    /// 实际下载中滚动平均的吞吐
    pub live_speed: Option<f64>,
    pub ok: u64,
    pub fail: u64,
    pub last_error: Option<String>,
    pub cooling: bool,
    pub tested_at: Option<u64>,
    #[serde(skip)]
    consecutive_fail: u32,
    #[serde(skip)]
    cooldown_until: Option<Instant>,
}

impl SourceStat {
    fn score(&self) -> f64 {
        if self.cooldown_until.map_or(false, |t| t > Instant::now()) {
            return 0.001;
        }
        let speed = self.live_speed.or(self.test_speed).unwrap_or(400_000.0);
        let success = (self.ok as f64 + 1.0) / ((self.ok + self.fail) as f64 + 2.0);
        // 只测过延迟、没测出速度的源（比如测速文件超时）给个保守值
        let latency_penalty = match self.latency_ms {
            Some(ms) if self.test_speed.is_none() && self.live_speed.is_none() => 1.0 / (1.0 + ms as f64 / 500.0),
            _ => 1.0,
        };
        speed * success * success * latency_penalty
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    #[serde(flatten)]
    pub def: SourceDef,
    #[serde(flatten)]
    pub stat: SourceStat,
    pub score: f64,
    pub rank: usize,
}

/// 一个待下载文件
#[derive(Debug, Clone)]
pub struct FileJob {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// 下载进度（多个文件共享）
#[derive(Default)]
pub struct Progress {
    pub cancel: AtomicBool,
    pub files_total: AtomicU64,
    pub files_done: AtomicU64,
    pub bytes_total: AtomicU64,
    pub bytes_done: AtomicU64,
    pub source_bytes: Mutex<HashMap<String, u64>>,
}

impl Progress {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled() {
            bail!("已取消");
        }
        Ok(())
    }
}

pub struct Net {
    client: reqwest::Client,
    settings: RwLock<NetSettings>,
    stats: Mutex<HashMap<String, SourceStat>>,
    /// 同时在传的文件数自动调：超时多了就收（网络或加速器扛不住 64 个连接时两个源会一起超时），
    /// 连续成功再慢慢放开，最多到设置里的「同时下载」
    gate: Gate,
}

struct Gate {
    sem: tokio::sync::Semaphore,
    /// 现在放行几个
    size: std::sync::atomic::AtomicUsize,
    /// 连续成功的次数
    streak: std::sync::atomic::AtomicUsize,
}

const GATE_START: usize = 16;
const GATE_MIN: usize = 4;

impl Gate {
    fn new() -> Gate {
        Gate { sem: tokio::sync::Semaphore::new(GATE_START), size: std::sync::atomic::AtomicUsize::new(GATE_START), streak: std::sync::atomic::AtomicUsize::new(0) }
    }

    fn ok(&self, max: usize) {
        let n = self.streak.fetch_add(1, Ordering::Relaxed) + 1;
        let cur = self.size.load(Ordering::Relaxed);
        // 每连续成功「当前并发数」次，放开一个
        if n >= cur && cur < max {
            self.streak.store(0, Ordering::Relaxed);
            self.size.fetch_add(1, Ordering::Relaxed);
            self.sem.add_permits(1);
        }
    }

    fn timeout(&self) {
        self.streak.store(0, Ordering::Relaxed);
        let cur = self.size.load(Ordering::Relaxed);
        if cur > GATE_MIN {
            let cut = (cur / 4).max(1).min(cur - GATE_MIN);
            let gone = self.sem.forget_permits(cut);
            self.size.fetch_sub(gone, Ordering::Relaxed);
        }
    }
}

// BMCLAPI 的路径改写规则：官方前缀 → 镜像下的子路径
const BMCL_RULES: &[(&str, &str)] = &[
    ("https://piston-meta.mojang.com", ""),
    ("https://launchermeta.mojang.com", ""),
    ("https://launcher.mojang.com", ""),
    ("https://piston-data.mojang.com", ""),
    ("https://libraries.minecraft.net", "/maven"),
    ("https://resources.download.minecraft.net", "/assets"),
    ("https://maven.minecraftforge.net", "/maven"),
    ("https://files.minecraftforge.net/maven", "/maven"),
    ("https://maven.neoforged.net/releases", "/maven"),
    ("https://meta.fabricmc.net", "/fabric-meta"),
    ("https://maven.fabricmc.net", "/maven"),
];

/// 老版本 JSON 里还有 http:// 的地址，统一成 https
fn normalize(url: &str) -> String {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{}", rest)
    } else {
        url.to_string()
    }
}

fn builtin_sources() -> Vec<SourceDef> {
    vec![
        SourceDef {
            id: "official".into(),
            name: "官方源".into(),
            kind: SourceKind::Official,
            base: String::new(),
            builtin: true,
            enabled: true,
        },
        SourceDef {
            id: "bmclapi".into(),
            name: "BMCLAPI 镜像".into(),
            kind: SourceKind::Bmclapi,
            base: BMCLAPI_BASE.into(),
            builtin: true,
            enabled: true,
        },
    ]
}

impl Net {
    pub fn new(settings: NetSettings) -> Result<Net> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(25))
            .pool_max_idle_per_host(32)
            .build()?;
        Ok(Net { client, settings: RwLock::new(settings), stats: Mutex::new(HashMap::new()), gate: Gate::new() })
    }

    pub fn settings(&self) -> NetSettings {
        self.settings.read().unwrap().clone()
    }

    pub fn set_settings(&self, s: NetSettings) {
        *self.settings.write().unwrap() = s;
    }

    pub fn threads(&self) -> usize {
        self.settings.read().unwrap().threads.clamp(1, 128)
    }

    pub fn sources(&self) -> Vec<SourceDef> {
        let s = self.settings.read().unwrap();
        builtin_sources().into_iter().chain(s.custom.iter().cloned()).filter(|d| d.enabled).collect()
    }

    /// 把官方地址改写成某个源的地址；该源不提供这个文件时返回 None
    pub fn rewrite(def: &SourceDef, url: &str) -> Option<String> {
        let url = normalize(url);
        match def.kind {
            SourceKind::Official => Some(url),
            SourceKind::Bmclapi => {
                let base = def.base.trim_end_matches('/');
                BMCL_RULES.iter().find_map(|(from, to)| {
                    url.strip_prefix(from)
                        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
                        .map(|rest| format!("{}{}{}", base, to, rest))
                })
            }
        }
    }

    fn ranked_sources(&self) -> Vec<SourceDef> {
        let settings = self.settings();
        let stats = self.stats.lock().unwrap();
        let mut list: Vec<(SourceDef, f64)> = self
            .sources()
            .into_iter()
            .map(|d| {
                let score = stats.get(&d.id).map(|s| s.score()).unwrap_or(400_000.0);
                (d, score)
            })
            .collect();
        list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if settings.mode == "fixed" {
            if let Some(pos) = list.iter().position(|(d, _)| d.id == settings.preferred) {
                let item = list.remove(pos);
                list.insert(0, item);
            }
        }
        list.into_iter().map(|(d, _)| d).collect()
    }

    /// 某个官方地址的候选下载地址，已按优先级排好
    pub fn candidates(&self, url: &str) -> Vec<(String, String)> {
        self.ranked_sources()
            .iter()
            .filter_map(|d| Net::rewrite(d, url).map(|u| (d.id.clone(), u)))
            .collect()
    }

    pub fn best_source_id(&self) -> String {
        self.ranked_sources().first().map(|d| d.id.clone()).unwrap_or_else(|| "official".into())
    }

    pub fn views(&self) -> Vec<SourceView> {
        let ranked = self.ranked_sources();
        let stats = self.stats.lock().unwrap();
        ranked
            .into_iter()
            .enumerate()
            .map(|(i, def)| {
                let mut stat = stats.get(&def.id).cloned().unwrap_or_default();
                stat.cooling = stat.cooldown_until.map_or(false, |t| t > Instant::now());
                let score = stat.score();
                SourceView { def, stat, score, rank: i + 1 }
            })
            .collect()
    }

    fn record_ok(&self, id: &str, bytes: u64, elapsed: Duration) {
        let mut stats = self.stats.lock().unwrap();
        let s = stats.entry(id.to_string()).or_default();
        s.ok += 1;
        s.consecutive_fail = 0;
        // 小文件主要是延迟，算速度不准，只统计 128KB 以上的
        if bytes >= 128 * 1024 {
            let speed = bytes as f64 / elapsed.as_secs_f64().max(0.001);
            s.live_speed = Some(match s.live_speed {
                Some(old) => old * 0.8 + speed * 0.2,
                None => speed,
            });
        }
    }

    fn record_fail(&self, id: &str, err: &str, throttled: bool) {
        let mut stats = self.stats.lock().unwrap();
        let s = stats.entry(id.to_string()).or_default();
        s.fail += 1;
        s.consecutive_fail += 1;
        s.last_error = Some(err.chars().take(160).collect());
        if throttled || s.consecutive_fail >= 4 {
            let secs = if throttled { 15 } else { 30 };
            s.cooldown_until = Some(Instant::now() + Duration::from_secs(secs));
            s.consecutive_fail = 0;
        }
    }

    /// 取元数据（版本列表、JSON 等），自动在各源之间回退
    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let cands = self.candidates(url);
        if cands.is_empty() {
            bail!("没有可用的下载源：{}", url);
        }
        let mut errors = vec![];
        // 网络时好时坏：所有源都失败了就歇一会儿再来一轮（镜像 404 的不再试）
        for round in 0..3 {
            if round > 0 {
                tokio::time::sleep(Duration::from_secs(2 * round)).await;
            }
            let mut retryable = false;
            for (id, u) in &cands {
                if round > 0 && errors.iter().any(|e: &String| e.starts_with(&format!("[{id}] HTTP 404"))) {
                    continue;
                }
                let t0 = Instant::now();
                match self.fetch_once(u).await {
                    Ok(b) => {
                        self.record_ok(id, b.len() as u64, t0.elapsed());
                        return Ok(b);
                    }
                    Err((e, throttled)) => {
                        self.record_fail(id, &e, throttled);
                        retryable |= !e.starts_with("HTTP 404");
                        errors.push(format!("[{}] {}", id, e));
                    }
                }
            }
            if !retryable {
                break;
            }
        }
        Err(anyhow!("全部下载源都失败了：{}", errors.join("；")))
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        serde_json::from_slice(&bytes).with_context(|| format!("解析 {} 失败", url))
    }

    /// 按给定顺序直接尝试几个地址（用于 BMCLAPI 独有的列表接口等）
    pub async fn get_json_from<T: DeserializeOwned>(&self, urls: &[String]) -> Result<T> {
        let mut errors = vec![];
        for u in urls {
            match self.fetch_once(u).await {
                Ok(b) => match serde_json::from_slice(&b) {
                    Ok(v) => return Ok(v),
                    Err(e) => errors.push(format!("{}：{}", u, e)),
                },
                Err((e, _)) => errors.push(format!("{}：{}", u, e)),
            }
        }
        Err(anyhow!("{}", errors.join("；")))
    }

    pub async fn get_text_from(&self, urls: &[String]) -> Result<String> {
        let mut errors = vec![];
        for u in urls {
            match self.fetch_once(u).await {
                Ok(b) => return Ok(String::from_utf8_lossy(&b).into_owned()),
                Err((e, _)) => errors.push(format!("{}：{}", u, e)),
            }
        }
        Err(anyhow!("{}", errors.join("；")))
    }

    async fn fetch_once(&self, url: &str) -> std::result::Result<Vec<u8>, (String, bool)> {
        let resp = tokio::time::timeout(Duration::from_secs(40), self.client.get(url).send())
            .await
            .map_err(|_| ("请求超时".to_string(), false))?
            .map_err(|e| (short_err(&e), false))?;
        let status = resp.status();
        if !status.is_success() {
            return Err((format!("HTTP {}", status.as_u16()), status.as_u16() == 429));
        }
        let body = tokio::time::timeout(Duration::from_secs(90), resp.bytes())
            .await
            .map_err(|_| ("读取超时".to_string(), false))?
            .map_err(|e| (short_err(&e), false))?;
        Ok(body.to_vec())
    }

    /// 下载单个文件：已存在且校验通过就跳过；按得分轮流尝试各源，最多 6 次
    pub async fn download(&self, job: &FileJob, prog: &Progress) -> Result<()> {
        prog.check()?;
        if file_ok(&job.path, job.sha1.as_deref(), job.size).await {
            prog.bytes_done.fetch_add(job.size.unwrap_or(0), Ordering::Relaxed);
            prog.files_done.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        if let Some(parent) = job.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut errors: Vec<String> = vec![];
        let mut attempt = 0usize;
        while attempt < 6 {
            prog.check()?;
            // 重试前歇一会儿，越往后歇得越久（两个源都在冷却时别一直撞）
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
            }
            let _permit = self.gate.sem.acquire().await.map_err(|_| anyhow!("下载器关了"))?;
            // 每次重新排序：前一次失败可能已经让那个源进了冷却
            let cands = self.candidates(&job.url);
            if cands.is_empty() {
                bail!("没有源能提供 {}", job.url);
            }
            let (id, url) = cands[attempt % cands.len()].clone();
            attempt += 1;
            let t0 = Instant::now();
            // 前两次太慢可以换源；再往后就是网本身慢，不换了，慢慢下完
            match self.stream_to_file(&url, job, prog, &id, attempt <= 2).await {
                Ok(n) => {
                    self.record_ok(&id, n, t0.elapsed());
                    self.gate.ok(self.threads());
                    prog.files_done.fetch_add(1, Ordering::Relaxed);
                    // 不知道大小的文件，下载完再补进总量，进度条不至于超过 100%
                    if job.size.is_none() {
                        prog.bytes_total.fetch_add(n, Ordering::Relaxed);
                    }
                    return Ok(());
                }
                Err(DlErr::Cancelled) => bail!("已取消"),
                Err(DlErr::Other(e, throttled)) => {
                    if throttled || e.contains("超时") || e.contains("停滞") || e.contains("太慢") {
                        self.gate.timeout();
                    }
                    self.record_fail(&id, &e, throttled);
                    errors.push(format!("[{}] {}", id, e));
                    if e.starts_with("HTTP 404") && cands.len() == 1 {
                        break;
                    }
                }
            }
        }
        let name = job.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Err(anyhow!("{} 下载失败：{}", name, errors.join("；")))
    }

    async fn stream_to_file(&self, url: &str, job: &FileJob, prog: &Progress, source: &str, may_switch: bool) -> std::result::Result<u64, DlErr> {
        let part = job.path.with_extension(format!(
            "{}part",
            job.path.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
        ));
        let mut received: u64 = 0;
        let result = async {
            let resp = tokio::time::timeout(Duration::from_secs(30), self.client.get(url).send())
                .await
                .map_err(|_| DlErr::Other("连接超时".into(), false))?
                .map_err(|e| DlErr::Other(short_err(&e), false))?;
            let status = resp.status();
            if !status.is_success() {
                return Err(DlErr::Other(format!("HTTP {}", status.as_u16()), status.as_u16() == 429));
            }
            let mut file = tokio::fs::File::create(&part).await.map_err(|e| DlErr::Other(e.to_string(), false))?;
            let mut hasher = Sha1::new();
            let mut stream = resp.bytes_stream();
            let started = Instant::now();
            // 有别的源可换时，太慢就换：传了 10 秒平均还不到 20 KB/s（只剩一个慢文件时最磨人）
            let can_switch = may_switch && self.candidates(&job.url).len() > 1;
            loop {
                if prog.cancelled() {
                    return Err(DlErr::Cancelled);
                }
                // 20 秒一个字节都没收到就当卡死，换源
                let next = tokio::time::timeout(Duration::from_secs(20), stream.next())
                    .await
                    .map_err(|_| DlErr::Other("传输停滞".into(), false))?;
                let Some(chunk) = next else { break };
                let chunk = chunk.map_err(|e| DlErr::Other(short_err(&e), false))?;
                hasher.update(&chunk);
                file.write_all(&chunk).await.map_err(|e| DlErr::Other(e.to_string(), false))?;
                received += chunk.len() as u64;
                prog.bytes_done.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                let secs = started.elapsed().as_secs_f64();
                if can_switch && secs > 10.0 && (received as f64) / secs < 20_000.0 && job.size.is_none_or(|s| s > received + 64 * 1024) {
                    return Err(DlErr::Other("太慢，换个源".into(), false));
                }
            }
            file.flush().await.map_err(|e| DlErr::Other(e.to_string(), false))?;
            drop(file);
            if let Some(expect) = &job.sha1 {
                let got = format!("{:x}", hasher.finalize());
                if !got.eq_ignore_ascii_case(expect) {
                    return Err(DlErr::Other(format!("校验失败（SHA1 {}…）", &got[..8]), false));
                }
            } else if let Some(size) = job.size {
                if size != received {
                    return Err(DlErr::Other(format!("大小不符（{} / {}）", received, size), false));
                }
            }
            tokio::fs::rename(&part, &job.path).await.map_err(|e| DlErr::Other(e.to_string(), false))?;
            Ok(received)
        }
        .await;
        match &result {
            Ok(n) => {
                *prog.source_bytes.lock().unwrap().entry(source.to_string()).or_insert(0) += n;
            }
            Err(_) => {
                // 失败的这次不计入进度
                prog.bytes_done.fetch_sub(received.min(prog.bytes_done.load(Ordering::Relaxed)), Ordering::Relaxed);
                let _ = tokio::fs::remove_file(&part).await;
            }
        }
        result
    }

    /// 测速：每个源取一次版本清单测延迟，再拉一个约 14MB 的库文件测 4 秒吞吐
    pub async fn speed_test(self: &Arc<Self>) {
        let defs = self.sources();
        let mut handles = vec![];
        for def in defs {
            let me = self.clone();
            handles.push(tokio::spawn(async move {
                let manifest = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
                let big = "https://libraries.minecraft.net/com/ibm/icu/icu4j/73.2/icu4j-73.2.jar";
                let mut latency = None;
                let mut speed = None;
                let mut err = None;
                if let Some(u) = Net::rewrite(&def, manifest) {
                    let t0 = Instant::now();
                    match tokio::time::timeout(Duration::from_secs(8), me.client.head(&u).send()).await {
                        Ok(Ok(r)) if r.status().is_success() || r.status().is_redirection() => {
                            latency = Some(t0.elapsed().as_millis() as u64)
                        }
                        Ok(Ok(r)) => err = Some(format!("HTTP {}", r.status().as_u16())),
                        Ok(Err(e)) => err = Some(short_err(&e)),
                        Err(_) => err = Some("超时".into()),
                    }
                }
                if latency.is_some() {
                    if let Some(u) = Net::rewrite(&def, big) {
                        // 镜像会 302 到不同节点，握手可能就要两三秒，预算给宽一点
                        speed = measure_throughput(&me.client, &u, Duration::from_secs(7), 6 * 1024 * 1024).await;
                    }
                }
                let mut stats = me.stats.lock().unwrap();
                let s = stats.entry(def.id.clone()).or_default();
                s.latency_ms = latency;
                s.test_speed = speed;
                s.tested_at = Some(crate::base::util::now_ms());
                if let Some(e) = err {
                    s.last_error = Some(e);
                    s.fail += 1;
                } else {
                    s.cooldown_until = None;
                }
            }));
        }
        for h in handles {
            let _ = h.await;
        }
    }
}

async fn measure_throughput(client: &reqwest::Client, url: &str, budget: Duration, cap: u64) -> Option<f64> {
    let t0 = Instant::now();
    let resp = tokio::time::timeout(budget, client.get(url).send()).await.ok()?.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body_start = Instant::now();
    let mut stream = resp.bytes_stream();
    let mut got: u64 = 0;
    while let Ok(Some(Ok(chunk))) = tokio::time::timeout(budget.saturating_sub(t0.elapsed()).max(Duration::from_millis(50)), stream.next()).await {
        got += chunk.len() as u64;
        if got >= cap || t0.elapsed() >= budget {
            break;
        }
    }
    if got < 64 * 1024 {
        return None;
    }
    Some(got as f64 / body_start.elapsed().as_secs_f64().max(0.05))
}

enum DlErr {
    Cancelled,
    Other(String, bool),
}

fn short_err(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "超时".into()
    } else if e.is_connect() {
        "连不上服务器".into()
    } else if let Some(s) = e.status() {
        format!("HTTP {}", s.as_u16())
    } else {
        let s = e.to_string();
        s.chars().take(120).collect()
    }
}

/// 文件已存在且（有 SHA1 时）校验通过
pub async fn file_ok(path: &Path, sha1: Option<&str>, size: Option<u64>) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else { return false };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }
    if let Some(sz) = size {
        if meta.len() != sz {
            return false;
        }
    }
    match sha1 {
        Some(expect) => {
            let p = path.to_path_buf();
            let expect = expect.to_string();
            tokio::task::spawn_blocking(move || sha1_file(&p).map(|h| h.eq_ignore_ascii_case(&expect)).unwrap_or(false))
                .await
                .unwrap_or(false)
        }
        None => true,
    }
}

pub fn sha1_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
