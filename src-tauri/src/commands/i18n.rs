//! 智能汉化（M11）：看每个 Mod 有没有中文、补全中文（社区汉化 + AI）、拿掉汉化包

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

use crate::base::task;
use crate::i18n::{self, ModStat, PACK};
use crate::state::{err, AppState, CmdResult};

fn where_is(st: &AppState, dir: &str, name: &str) -> CmdResult<(std::path::PathBuf, std::path::PathBuf, String, String)> {
    crate::instance::check_name(name).map_err(err)?;
    let root = crate::commands::instance::root_of(st, dir)?;
    let gd = crate::instance::game_dir(&root, name).0;
    let t = st.targets.lock().unwrap();
    let v = t.iter().find(|t| t.id == dir).and_then(|t| t.versions.iter().find(|v| v.name == name)).ok_or("找不到这个实例")?;
    Ok((root.clone(), gd, v.mc.clone().unwrap_or_default(), v.loader.to_lowercase()))
}

fn manifest(gd: &std::path::Path) -> Value {
    std::fs::read(gd.join("resourcepacks").join(PACK).join("leafstarter.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

fn stats(mods: &[i18n::ModLang], man: &Value) -> Vec<ModStat> {
    mods.iter()
        .map(|m| {
            let mut s = ModStat { file: m.file.clone(), namespaces: m.ns.keys().cloned().collect(), ..Default::default() };
            for (ns, (en, zh)) in &m.ns {
                // 英文是空字符串的不算
                let en: BTreeMap<&String, &String> = en.iter().filter(|(_, v)| !v.trim().is_empty()).collect();
                s.total += en.len();
                s.own += en.keys().filter(|k| zh.contains_key(**k)).count();
                let miss: Vec<&String> = en.keys().filter(|k| !zh.contains_key(**k)).copied().collect();
                let (c, a) = (man["ns"][ns]["community"].as_u64().unwrap_or(0) as usize, man["ns"][ns]["ai"].as_u64().unwrap_or(0) as usize);
                s.community += c.min(miss.len());
                s.ai += a.min(miss.len().saturating_sub(c));
                let left = miss.len().saturating_sub(c + a);
                s.missing += left;
                s.missing_chars += miss.iter().rev().take(left).map(|k| en[*k].chars().count()).sum::<usize>();
            }
            s
        })
        .collect()
}

/// 每个 Mod 的中文情况，加上汉化包和翻译用的 AI 源
#[tauri::command]
pub async fn i18n_scan(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<Value> {
    let (_, gd, mc, _) = where_is(&state, &dir, &name)?;
    let tr = state.settings.lock().unwrap().ai.for_use("tr").map(|s| format!("{} · {}", s.name, s.model));
    let gd2 = gd.clone();
    let (list, man) = tauri::async_runtime::spawn_blocking(move || {
        let mods = i18n::read_mods(&gd2.join("mods"));
        let man = manifest(&gd2);
        (stats(&mods, &man), man)
    })
    .await
    .map_err(|e| e.to_string())?;
    let sum = |f: fn(&ModStat) -> usize| list.iter().map(f).sum::<usize>();
    Ok(json!({
        "mc": mc, "mods": list,
        "total": sum(|m| m.total), "own": sum(|m| m.own), "community": sum(|m| m.community), "ai": sum(|m| m.ai),
        "missing": sum(|m| m.missing), "missingChars": sum(|m| m.missing_chars),
        "pack": gd.join("resourcepacks").join(PACK).is_dir(), "at": man["at"], "trSource": tr,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOpts {
    pub community: bool,
    pub ai: bool,
    /// 只翻这几个 Mod（文件名）；空的表示全部
    #[serde(default)]
    pub files: Vec<String>,
}

#[tauri::command]
pub async fn i18n_run(app: AppHandle, state: State<'_, AppState>, dir: String, name: String, opts: RunOpts) -> CmdResult<u64> {
    let (root, gd, mc, loader) = where_is(&state, &dir, &name)?;
    if mc.is_empty() {
        return Err("认不出这个实例的游戏版本".into());
    }
    let src = state.settings.lock().unwrap().ai.for_use("tr").cloned();
    if opts.ai && src.is_none() {
        return Err("要用 AI 补的话，先到设置里加一个 AI 源".into());
    }
    let task = state.tasks.create(format!("给「{name}」补中文"), mc.clone(), String::new());
    let tid = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let data_dir = state.data_dir.clone();
    let net = state.net.clone();
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            // 哪一层没成、为什么：一条都没补上时整个任务算失败，原因直接给玩家看
            let mut problems: Vec<String> = vec![];
            task.stage("读 Mod 里的语言文件");
            let gd2 = gd.clone();
            let files = opts.files.clone();
            let mods = tokio::task::spawn_blocking(move || i18n::read_mods(&gd2.join("mods")).into_iter().filter(|m| files.is_empty() || files.contains(&m.file)).collect::<Vec<_>>()).await?;
            // 每个命名空间缺的条目（英文）和属于哪个 Mod
            let mut missing: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
            let mut owner: HashMap<String, String> = HashMap::new();
            for m in &mods {
                for (ns, (en, zh)) in &m.ns {
                    let e = missing.entry(ns.clone()).or_default();
                    for (k, v) in en {
                        if !zh.contains_key(k) && !v.trim().is_empty() {
                            e.insert(k.clone(), v.clone());
                        }
                    }
                    owner.entry(ns.clone()).or_insert_with(|| m.file.trim_end_matches(".jar").to_string());
                }
            }
            missing.retain(|_, m| !m.is_empty());
            let need: usize = missing.values().map(|m| m.len()).sum();
            task.log(format!("{} 个 Mod、{} 个命名空间，缺中文的 {need} 条", mods.len(), missing.len()));
            if need == 0 {
                return Ok(Some("这些 Mod 都自带中文，不用补".into()));
            }
            let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
            let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
            let leg = i18n::legacy(&mc);
            // 第二层：社区汉化
            if opts.community && need > 0 {
                let names = i18n::cfpa_assets(&mc, &loader);
                if names.is_empty() {
                    task.log("社区汉化没有这个游戏版本的包");
                    problems.push(format!("社区汉化没有 {mc} 的包"));
                }
                let before = counts.values().map(|c| c.0).sum::<usize>();
                let nss: Vec<String> = missing.keys().cloned().collect();
                for n in names {
                    task.stage(format!("下载社区汉化（CFPA）：{n}"));
                    match fetch_cfpa(&net, &task, &data_dir, &n).await {
                        Ok(path) => {
                            let got = i18n::cfpa_read(&path, &nss, leg);
                            for (ns, map) in got {
                                let miss = missing.get_mut(&ns).unwrap();
                                for (k, v) in map {
                                    if miss.contains_key(&k) && !out.get(&ns).is_some_and(|o| o.contains_key(&k)) {
                                        miss.remove(&k);
                                        out.entry(ns.clone()).or_default().insert(k, v);
                                        counts.entry(ns.clone()).or_default().0 += 1;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            task.log(format!("社区汉化下载失败：{e:#}"));
                            problems.push(format!("社区汉化包没下下来（{e:#}）"));
                        }
                    }
                }
                missing.retain(|_, m| !m.is_empty());
                let got = counts.values().map(|c| c.0).sum::<usize>() - before;
                task.log(format!("社区汉化补了 {got} 条"));
                if got == 0 && !problems.iter().any(|p| p.starts_with("社区汉化")) {
                    problems.push("社区汉化里没有这些 Mod 的翻译".into());
                }
            }
            // 第三层：AI
            let mut tokens = 0u64;
            let mut bad = 0usize;
            if opts.ai {
                if let Some(src) = src {
                    task.stage("对原版术语表");
                    let (r2, n2, m2) = (root.clone(), name.clone(), mc.clone());
                    let gl = Arc::new(tokio::task::spawn_blocking(move || i18n::vanilla_glossary(&r2, &n2, &m2)).await?);
                    task.log(format!("原版术语 {} 条", gl.len()));
                    let cache = Arc::new(Mutex::new(i18n::Cache::load(&data_dir)));
                    // 先用缓存，剩下的分批
                    let mut batches: Vec<(String, String, Vec<(String, String)>)> = vec![];
                    for (ns, miss) in &missing {
                        let mut cur: Vec<(String, String)> = vec![];
                        let mut chars = 0;
                        for (k, v) in miss {
                            if let Some(z) = cache.lock().unwrap().get(v) {
                                out.entry(ns.clone()).or_default().insert(k.clone(), z.clone());
                                counts.entry(ns.clone()).or_default().1 += 1;
                                continue;
                            }
                            chars += v.len();
                            cur.push((k.clone(), v.clone()));
                            if cur.len() >= 60 || chars > 4000 {
                                batches.push((ns.clone(), owner[ns].clone(), std::mem::take(&mut cur)));
                                chars = 0;
                            }
                        }
                        if !cur.is_empty() {
                            batches.push((ns.clone(), owner[ns].clone(), cur));
                        }
                    }
                    task.stage(format!("AI 翻译（{} 批）", batches.len()));
                    task.prog.files_total.store(batches.len() as u64, Ordering::Relaxed);
                    let http = crate::agent::llm::client()?;
                    let results: Vec<(String, Vec<(String, String, String)>, u64, Option<String>)> = futures_util::stream::iter(batches.into_iter().map(|(ns, owner, entries)| {
                        let (http, src, gl, prog) = (http.clone(), src.clone(), gl.clone(), task.prog.clone());
                        async move {
                            let texts: Vec<&str> = entries.iter().map(|(_, v)| v.as_str()).collect();
                            let terms = i18n::pick_terms(&gl, &texts);
                            let prompt = i18n::batch_prompt(&owner, &terms, &entries);
                            let r = crate::agent::llm::complete(&http, &src, i18n::SYSTEM, &prompt).await;
                            prog.files_done.fetch_add(1, Ordering::Relaxed);
                            match r {
                                Ok((text, t)) => {
                                    let got = i18n::parse_reply(&text);
                                    let triples = entries.iter().filter_map(|(k, en)| got.get(k).map(|z| (k.clone(), en.clone(), z.clone()))).collect();
                                    (ns, triples, t, None)
                                }
                                Err(e) => (ns, vec![], 0, Some(format!("{e:#}"))),
                            }
                        }
                    }))
                    .buffer_unordered(3)
                    .collect()
                    .await;
                    let mut errs = vec![];
                    for (ns, triples, t, e) in results {
                        tokens += t;
                        if let Some(e) = e {
                            errs.push(e);
                        }
                        for (k, en, zh) in triples {
                            if zh.trim().is_empty() || !i18n::same_placeholders(&en, &zh) {
                                bad += 1;
                                continue;
                            }
                            cache.lock().unwrap().put(en, zh.clone());
                            out.entry(ns.clone()).or_default().insert(k, zh);
                            counts.entry(ns.clone()).or_default().1 += 1;
                        }
                    }
                    cache.lock().unwrap().save();
                    let ai_got: usize = counts.values().map(|c| c.1).sum();
                    if ai_got == 0 && errs.is_empty() && need > 0 {
                        problems.push(if bad > 0 { format!("AI 翻的 {bad} 条格式符都对不上") } else { "AI 回的内容认不出来（换个模型试试）".into() });
                    }
                    if let Some(e) = errs.first() {
                        task.log(format!("有 {} 批失败：{e}", errs.len()));
                        problems.push(format!("AI 有 {} 批没翻成：{e}", errs.len()));
                    }
                }
            }
            let (c, a): (usize, usize) = counts.values().fold((0, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1));
            if c + a == 0 {
                if problems.is_empty() {
                    anyhow::bail!("一条都没补上");
                }
                anyhow::bail!("一条都没补上：{}", problems.join("；"));
            }
            task.stage("生成汉化资源包");
            let man = json!({ "mc": mc, "at": crate::base::util::now_ms(), "ns": counts.iter().map(|(k, (c, a))| (k.clone(), json!({ "community": c, "ai": a }))).collect::<serde_json::Map<_, _>>(), "tokens": tokens });
            let pack_dir = gd.join("resourcepacks").join(PACK);
            let (o2, m2, mc2) = (out.clone(), man.clone(), mc.clone());
            tokio::task::spawn_blocking(move || i18n::write_pack(&pack_dir, &mc2, &o2, &m2)).await??;
            let lang = i18n::enable_pack(&gd, &mc)?;
            let left = need.saturating_sub(c + a);
            let mut msg = format!("补了 {} 条：社区汉化 {c} 条、AI {a} 条", c + a);
            if left > 0 {
                msg.push_str(&format!("；还有 {left} 条没中文"));
            }
            if bad > 0 {
                msg.push_str(&format!("（AI 有 {bad} 条格式符对不上，退回英文）"));
            }
            if !problems.is_empty() {
                msg.push_str(&format!("。{}", problems.join("；")));
            }
            if tokens > 0 {
                msg.push_str(&format!("，用了约 {} 万 token", (tokens as f64 / 1e4 * 10.0).round() / 10.0));
            }
            if !lang.to_lowercase().starts_with("zh") {
                msg.push_str("。游戏语言现在不是中文，进游戏在「设置 → 语言」里选简体中文");
            }
            Ok(Some(msg))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("i18n-done", json!({ "dir": dir, "name": name }));
    });
    Ok(tid)
}

/// CFPA 的包缓存 3 天；过期了下不下来就先用旧的。走统一的下载器（会重试，GitHub 慢时分段下）
async fn fetch_cfpa(net: &Arc<crate::base::net::Net>, task: &Arc<crate::base::task::TaskEntry>, data_dir: &std::path::Path, file: &str) -> anyhow::Result<std::path::PathBuf> {
    let p = data_dir.join("i18n").join("cfpa").join(file);
    let fresh = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs() < 3 * 86400);
    if fresh {
        return Ok(p);
    }
    let tmp = p.with_extension("new");
    let _ = std::fs::remove_file(&tmp);
    let job = crate::base::net::FileJob { url: format!("{}/{file}", i18n::CFPA_RELEASE), path: tmp.clone(), sha1: None, size: None };
    task.reset_counters();
    match net.download(&job, &task.prog).await {
        Ok(()) => {
            std::fs::rename(&tmp, &p).or_else(|_| crate::base::util::move_file(&tmp, &p))?;
            Ok(p)
        }
        Err(e) if p.is_file() => {
            task.log(format!("社区汉化更新失败，先用上次下的：{e:#}"));
            Ok(p)
        }
        Err(e) => Err(e),
    }
}

/// 拿掉汉化包（放进回收站）
#[tauri::command]
pub fn i18n_remove(state: State<'_, AppState>, dir: String, name: String) -> CmdResult<()> {
    let (_, gd, _, _) = where_is(&state, &dir, &name)?;
    i18n::disable_pack(&gd).map_err(err)?;
    let p = gd.join("resourcepacks").join(PACK);
    if p.is_dir() {
        crate::instance::to_recycle_bin(&p).map_err(err)?;
    }
    Ok(())
}
