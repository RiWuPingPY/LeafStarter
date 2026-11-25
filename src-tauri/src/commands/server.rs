//! 常驻服务器（M10）：加服务器（密码换密钥）、跑开服剧本、启停、控制台、备份、搬世界、开云防火墙、面板服

use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

use crate::base::task;
use crate::server::{self, panel, playbook, rcon, ssh, Entry};
use crate::state::{err, AppState, CmdResult};

fn data(st: &AppState) -> PathBuf {
    st.data_dir.clone()
}

#[tauri::command]
pub fn server_list(state: State<'_, AppState>) -> Vec<Value> {
    server::load(&data(&state)).iter().map(Entry::view).collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSsh {
    pub name: String,
    pub host: String,
    pub ssh_port: Option<u16>,
    pub user: String,
    pub password: Option<String>,
    pub key_text: Option<String>,
    /// 对应的本地实例 dir/name
    pub inst: String,
}

/// 本地实例的 MC 版本、加载器（小写）、加载器版本
fn inst_info(st: &AppState, inst: &str) -> CmdResult<(String, String, String)> {
    let (dir, name) = inst.split_once('/').ok_or("实例 id 不对")?;
    let t = st.targets.lock().unwrap();
    let v = t.iter().find(|t| t.id == dir).and_then(|t| t.versions.iter().find(|v| v.name == name)).ok_or("找不到这个实例")?;
    let mc = v.mc.clone().ok_or("认不出这个实例的游戏版本")?;
    Ok((mc, v.loader.to_lowercase(), v.loader_version.clone().unwrap_or_default()))
}

/// 加一台 SSH 服务器：用密码（只用这一次）或用户的私钥登录，装上我们的密钥
#[tauri::command]
pub async fn server_add_ssh(state: State<'_, AppState>, form: AddSsh) -> CmdResult<Value> {
    let host = form.host.trim().to_string();
    if host.is_empty() {
        return Err("填一下服务器 IP".into());
    }
    let user = if form.user.trim().is_empty() { "root".to_string() } else { form.user.trim().to_string() };
    let port = form.ssh_port.unwrap_or(22);
    let auth = match (form.password.filter(|p| !p.is_empty()), form.key_text.filter(|k| !k.trim().is_empty())) {
        (_, Some(k)) => ssh::Auth::Key(k),
        (Some(p), None) => ssh::Auth::Password(p),
        _ => return Err("填密码，或者选云控制台下载的密钥文件".into()),
    };
    let (mc, loader, lv) = inst_info(&state, &form.inst)?;
    let (key, fp) = server::enroll(&host, port, &user, auth).await.map_err(err)?;
    let e = Entry {
        id: server::token(8).to_lowercase(),
        name: if form.name.trim().is_empty() { host.clone() } else { form.name.trim().to_string() },
        kind: "ssh".into(),
        host,
        ssh_port: port,
        user,
        fingerprint: fp,
        key,
        inst: form.inst,
        mc,
        loader,
        loader_version: lv,
        mem_mb: 0,
        port: 25565,
        rcon_port: 25575,
        rcon_pass: crate::auth::vault::seal(server::token(20).as_bytes()).map_err(err)?,
        online: true,
        created: crate::base::util::now_ms(),
        ..Default::default()
    };
    server::put(&data(&state), &e).map_err(err)?;
    Ok(e.view())
}

/// 改名字、内存、端口、正版验证、白名单、MOTD（下次跑剧本时生效）
#[tauri::command]
pub fn server_set(state: State<'_, AppState>, id: String, patch: Value) -> CmdResult<Value> {
    let e = server::update(&data(&state), &id, |e| {
        if let Some(v) = patch["name"].as_str() {
            e.name = v.trim().to_string();
        }
        if let Some(v) = patch["memMb"].as_u64() {
            e.mem_mb = v as u32;
        }
        if let Some(v) = patch["port"].as_u64() {
            e.port = v as u16;
        }
        if let Some(v) = patch["online"].as_bool() {
            e.online = v;
        }
        if let Some(v) = patch["motd"].as_str() {
            e.motd = v.to_string();
        }
        if let Some(v) = patch["whitelist"].as_array() {
            e.whitelist = v.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        }
    })
    .map_err(err)?;
    Ok(e.view())
}

#[tauri::command]
pub fn server_remove(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    server::remove(&data(&state), &id).map_err(err)
}

/// 指纹变了（比如重装了系统）：用户确认后重新信任
#[tauri::command]
pub async fn server_retrust(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let c = ssh::connect(&e.host, e.ssh_port, &e.user, &e.auth().map_err(err)?, None).await.map_err(err)?;
    let fp = c.fingerprint.clone();
    c.close().await;
    server::update(&data(&state), &id, |e| e.fingerprint = fp.clone()).map_err(err)?;
    Ok(fp)
}

#[tauri::command]
pub async fn server_harden(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let c = e.connect().await.map_err(err)?;
    let o = c.script(server::DISABLE_PASSWORD, |_| {}).await.map_err(err)?;
    c.close().await;
    if o.code != 0 {
        return Err(format!("没关成：{}", ssh::tail(&o.text, 4)));
    }
    Ok(ssh::tail(&o.text, 1))
}

fn emit_step(app: &AppHandle, id: &str, step: &str, title: &str, state: &str, out: &str) {
    let _ = app.emit("server-step", json!({ "id": id, "step": step, "title": title, "state": state, "out": out }));
}

/// 跑开服剧本（全部步骤都能重复执行；`only` 只跑某几步，比如只同步 Mod）
#[tauri::command]
pub async fn server_setup(app: AppHandle, state: State<'_, AppState>, id: String, only: Option<Vec<String>>) -> CmdResult<u64> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    if e.kind != "ssh" {
        return Err("面板服不用跑剧本".into());
    }
    let title = if only.is_some() { format!("同步到「{}」", e.name) } else { format!("开服：{}", e.name) };
    let task = state.tasks.create(title, format!("{} · {}", e.host, e.mc), String::new());
    let tid = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let net = state.net.clone();
    let dd = data(&state);
    let gd = local_game_dir(&state, &e.inst);
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            task.stage("算服务端版本和下载地址");
            let plan = server::make_plan(&net, &e).await?;
            task.stage(format!("连 {}", e.host));
            let c = e.connect().await?;
            let mut last = String::new();
            for st in playbook::steps(&plan) {
                if only.as_ref().is_some_and(|o| !o.iter().any(|x| x == st.id)) {
                    continue;
                }
                task.stage(st.title);
                emit_step(&app, &e.id, st.id, st.title, "run", "");
                let res: anyhow::Result<String> = if st.id == playbook::UPLOAD_MODS {
                    match &gd {
                        Some(gd) => server::sync_mods(&c, &e, gd, &|l| task.log(l)).await,
                        None => Ok("找不到本地实例，跳过".into()),
                    }
                } else {
                    let o = c.script(&st.script, |l| task.log(l.to_string())).await?;
                    if o.code == 0 { Ok(ssh::tail(&o.text, 2)) } else { Err(anyhow::anyhow!("{}", ssh::tail(&o.text, 30))) }
                };
                match res {
                    Ok(out) => {
                        emit_step(&app, &e.id, st.id, st.title, "done", &out);
                        let sid = st.id.to_string();
                        server::update(&dd, &e.id, |x| {
                            if !x.done.contains(&sid) {
                                x.done.push(sid.clone());
                            }
                            x.failed = None;
                        })?;
                        last = out;
                    }
                    Err(err) => {
                        let out = format!("{err:#}");
                        emit_step(&app, &e.id, st.id, st.title, "fail", &out);
                        let sid = st.id.to_string();
                        server::update(&dd, &e.id, |x| x.failed = Some((sid, out.clone())))?;
                        c.close().await;
                        anyhow::bail!("「{}」这一步失败了", st.title);
                    }
                }
            }
            // 白名单：服务器起来以后用 RCON 加
            let e2 = server::get(&dd, &e.id)?;
            if only.is_none() && !e2.whitelist.is_empty() {
                task.stage("加白名单");
                if let Ok(mut r) = rcon_login(&c, &e2).await {
                    for n in &e2.whitelist {
                        let out = r.cmd(&format!("whitelist add {n}")).await.unwrap_or_default();
                        task.log(out);
                    }
                }
            }
            c.close().await;
            Ok(Some(if only.is_none() { format!("开好了。朋友在多人游戏里添加服务器：{}", e2.view()["address"].as_str().unwrap_or("")) } else { last }))
        }
        .await;
        task::finish(&app, &task, r);
        let _ = app.emit("servers-changed", json!({ "id": id }));
    });
    Ok(tid)
}

fn local_game_dir(st: &AppState, inst: &str) -> Option<PathBuf> {
    let (dir, name) = inst.split_once('/')?;
    let root = crate::commands::instance::root_of(st, dir).ok()?;
    Some(crate::instance::game_dir(&root, name).0)
}

async fn rcon_login(c: &ssh::Conn, e: &Entry) -> anyhow::Result<rcon::Rcon<russh::ChannelStream<russh::client::Msg>>> {
    let s = c.tunnel(e.rcon_port).await?;
    rcon::Rcon::login(s, &e.rcon_password()?).await
}

/// start / stop / restart
#[tauri::command]
pub async fn server_power(state: State<'_, AppState>, id: String, action: String) -> CmdResult<String> {
    if !["start", "stop", "restart", "kill"].contains(&action.as_str()) {
        return Err("不认识的操作".into());
    }
    let e = server::get(&data(&state), &id).map_err(err)?;
    if e.kind != "ssh" {
        e.panel().map_err(err)?.power(&e.panel_inst, &e.panel_daemon, &action).await.map_err(err)?;
        return Ok("面板收到了".into());
    }
    let c = e.connect().await.map_err(err)?;
    let o = c.exec(&playbook::ctl(&e.id, if action == "kill" { "stop" } else { &action })).await.map_err(err)?;
    c.close().await;
    if o.code != 0 {
        return Err(ssh::tail(&o.text, 6));
    }
    Ok(match action.as_str() {
        "start" => "启动了（世界加载要一会儿）",
        "stop" => "停了",
        _ => "重启了",
    }
    .into())
}

/// 状态：在不在运行、在线人数、内存、负载
#[tauri::command]
pub async fn server_status(state: State<'_, AppState>, id: String) -> CmdResult<Value> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    if e.kind != "ssh" {
        let s = e.panel().map_err(err)?.status(&e.panel_inst, &e.panel_daemon).await.map_err(err)?;
        return Ok(json!({ "state": s.state, "memMb": s.memory_mb, "cpu": s.cpu }));
    }
    let c = e.connect().await.map_err(err)?;
    let b = playbook::base(&e.id);
    let svc = format!("leafstarter-{}", e.id);
    let script = format!(
        r#"st=$(bash "{b}/ctl.sh" status 2>/dev/null || echo none)
echo "state=$st"
if [ -f "/etc/systemd/system/{svc}.service" ]; then m=$(systemctl show -p MemoryCurrent --value {svc} 2>/dev/null); else p=$(cat "{b}/server.pid" 2>/dev/null); m=$(ps -o rss= --ppid "$p" -p "$p" 2>/dev/null | awk '{{s+=$1}} END {{print s*1024}}'); fi
echo "mem=$m"
echo "load=$(cut -d' ' -f1 /proc/loadavg)"
echo "total=$(free -m | awk '/^Mem/{{print $2}}')"
echo "disk=$(df -Pm "$HOME" | awk 'NR==2{{print $4}}')""#
    );
    let o = c.script(&script, |_| {}).await.map_err(err)?;
    let kv: std::collections::HashMap<&str, &str> = o.text.lines().filter_map(|l| l.split_once('=')).collect();
    let st = kv.get("state").copied().unwrap_or("none");
    let mut v = json!({
        "state": match st { "active" => "running", "inactive" => "stopped", "none" => "notSetup", _ => "unknown" },
        "memMb": kv.get("mem").and_then(|m| m.parse::<u64>().ok()).map(|b| b / 1048576),
        "load": kv.get("load").and_then(|m| m.parse::<f64>().ok()),
        "totalMb": kv.get("total").and_then(|m| m.parse::<u64>().ok()),
        "diskMb": kv.get("disk").and_then(|m| m.parse::<u64>().ok()),
    });
    if st == "active" {
        if let Ok(mut r) = rcon_login(&c, &e).await {
            if let Ok(out) = r.cmd("list").await {
                if let Some((n, max, names)) = rcon::parse_list(&out) {
                    v["players"] = json!(n);
                    v["maxPlayers"] = json!(max);
                    v["names"] = json!(names);
                }
            }
            // Paper / Purpur 有 tps 命令，原版没有
            if let Ok(out) = r.cmd("tps").await {
                if out.contains("TPS") {
                    v["tps"] = json!(out.trim());
                }
            }
        }
    }
    c.close().await;
    Ok(v)
}

/// 控制台：发一条命令
#[tauri::command]
pub async fn server_cmd(state: State<'_, AppState>, id: String, command: String) -> CmdResult<String> {
    let command = command.trim().trim_start_matches('/').to_string();
    if command.is_empty() {
        return Err("命令是空的".into());
    }
    let e = server::get(&data(&state), &id).map_err(err)?;
    if e.kind != "ssh" {
        e.panel().map_err(err)?.command(&e.panel_inst, &e.panel_daemon, &command).await.map_err(err)?;
        return Ok("发过去了（输出看控制台日志）".into());
    }
    let c = e.connect().await.map_err(err)?;
    let mut r = rcon_login(&c, &e).await.map_err(|e| format!("连不上 RCON（服务器没在运行？）：{e:#}"))?;
    let out = r.cmd(&command).await.map_err(err)?;
    c.close().await;
    Ok(if out.trim().is_empty() { "（没有输出）".into() } else { out })
}

#[tauri::command]
pub async fn server_log(state: State<'_, AppState>, id: String, lines: Option<u32>) -> CmdResult<String> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    if e.kind != "ssh" {
        return e.panel().map_err(err)?.log(&e.panel_inst, &e.panel_daemon).await.map_err(err);
    }
    let c = e.connect().await.map_err(err)?;
    let o = c.exec(&playbook::ctl(&e.id, &format!("log {}", lines.unwrap_or(200).min(2000)))).await.map_err(err)?;
    c.close().await;
    Ok(o.text)
}

/// 马上备份一次（在运行的话先让它把世界存盘）
#[tauri::command]
pub async fn server_backup(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let c = e.connect().await.map_err(err)?;
    let mut r = rcon_login(&c, &e).await.ok();
    if let Some(r) = r.as_mut() {
        let _ = r.cmd("save-off").await;
        let _ = r.cmd("save-all flush").await;
    }
    let o = c.exec(&playbook::ctl(&e.id, "backup")).await;
    if let Some(r) = r.as_mut() {
        let _ = r.cmd("save-on").await;
    }
    c.close().await;
    let o = o.map_err(err)?;
    if o.code != 0 {
        return Err(ssh::tail(&o.text, 4));
    }
    Ok(format!("备份好了：{}", o.text.trim().rsplit('/').next().unwrap_or("")))
}

#[tauri::command]
pub async fn server_backups(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Value>> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let c = e.connect().await.map_err(err)?;
    let o = c.exec(&playbook::ctl(&e.id, "backups")).await.map_err(err)?;
    c.close().await;
    Ok(o.text.lines().filter_map(|l| l.rsplit_once(' ')).map(|(n, s)| json!({ "name": n, "size": s.parse::<u64>().unwrap_or(0) })).collect())
}

/// 把本地一个世界传上去当服务器的世界（服务器上原来的先备份）
#[tauri::command]
pub async fn server_world_up(app: AppHandle, state: State<'_, AppState>, id: String, world: String) -> CmdResult<u64> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let gd = local_game_dir(&state, &e.inst).ok_or("找不到本地实例")?;
    crate::instance::check_name(&world).map_err(err)?;
    let src = gd.join("saves").join(&world);
    if !src.join("level.dat").is_file() {
        return Err("这个世界没有 level.dat".into());
    }
    let task = state.tasks.create(format!("把「{world}」传到「{}」", e.name), e.host.clone(), String::new());
    let tid = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let tmp = state.data_dir.join("tmp").join(format!("up-{}.tar.gz", e.id));
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            task.stage("打包世界");
            std::fs::create_dir_all(tmp.parent().unwrap())?;
            let (s2, t2) = (src.clone(), tmp.clone());
            tokio::task::spawn_blocking(move || server::pack_dir(&s2, &t2, Some("world"))).await??;
            let size = std::fs::metadata(&tmp)?.len();
            task.stage(format!("上传（{} MB）", size / 1048576));
            let c = e.connect().await?;
            let b = playbook::base(&e.id);
            let f = tokio::fs::File::open(&tmp).await?;
            c.upload(f, &format!("{b}/tmp/world-up.tar.gz")).await?;
            let _ = std::fs::remove_file(&tmp);
            task.stage("换上新世界（原来的先备份）");
            let script = format!(
                r#"B="{b}"; S="$B/server"; W=$(bash "$B/ctl.sh" world); was=$(bash "$B/ctl.sh" status)
bash "$B/ctl.sh" stop
if [ -d "$S/$W" ]; then mkdir -p "$B/backups"; tar czf "$B/backups/$W-before-upload-$(date +%Y%m%d-%H%M%S).tar.gz" --exclude=session.lock -C "$S" "$W"; rm -rf "$S/$W"; fi
mkdir -p "$S/$W"; tar xzf "$B/tmp/world-up.tar.gz" -C "$S/$W" --strip-components=1; rm -f "$B/tmp/world-up.tar.gz"
[ "$was" = active ] && bash "$B/ctl.sh" start
echo "换好了"
"#
            );
            let o = c.script(&script, |l| task.log(l.to_string())).await?;
            c.close().await;
            if o.code != 0 {
                anyhow::bail!("{}", ssh::tail(&o.text, 8));
            }
            Ok(Some("传好了。单人存档里你自己的背包在 level.dat 里，服务器不认，进去后背包是空的".into()))
        }
        .await;
        task::finish(&app, &task, r);
    });
    Ok(tid)
}

/// 把服务器的世界拉回本地实例（另存一份，不覆盖本地的）
#[tauri::command]
pub async fn server_world_down(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<u64> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let gd = local_game_dir(&state, &e.inst).ok_or("找不到本地实例")?;
    let task = state.tasks.create(format!("把「{}」的世界拉回来", e.name), e.host.clone(), String::new());
    let tid = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    let tmp = state.data_dir.join("tmp").join(format!("down-{}.tar.gz", e.id));
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            let c = e.connect().await?;
            let b = playbook::base(&e.id);
            // 在运行的话先存盘
            if let Ok(mut r) = rcon_login(&c, &e).await {
                let _ = r.cmd("save-all flush").await;
            }
            task.stage("打包、下载");
            std::fs::create_dir_all(tmp.parent().unwrap())?;
            let mut f = tokio::fs::File::create(&tmp).await?;
            let prog = task.prog.clone();
            c.download_cmd(&format!(r#"S="{b}/server"; W=$(bash "{b}/ctl.sh" world); cd "$S" && tar czf - --exclude=session.lock "$W""#), &mut f, |n| prog.bytes_done.store(n, std::sync::atomic::Ordering::Relaxed)).await?;
            drop(f);
            c.close().await;
            task.stage("解开到本地");
            let day = crate::base::util::now_ms() / 1000;
            let name = format!("{} 的世界 {}", e.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"), day % 100000);
            let dest = gd.join("saves").join(&name);
            let (t2, d2) = (tmp.clone(), dest.clone());
            let n = tokio::task::spawn_blocking(move || server::unpack(&t2, &d2, true)).await??;
            let _ = std::fs::remove_file(&tmp);
            Ok(Some(format!("拉回来了：存档「{name}」（{n} 个文件）")))
        }
        .await;
        task::finish(&app, &task, r);
    });
    Ok(tid)
}

/// 调云接口放行端口（AccessKey 只用这一次，不保存）
#[tauri::command]
pub async fn server_firewall(state: State<'_, AppState>, id: String, provider: String, ak: String, sk: String) -> CmdResult<String> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let port = if e.port == 0 { 25565 } else { e.port };
    server::cloud::open_port(&provider, ak.trim(), sk.trim(), &e.host, port).await.map_err(err)
}

// ---------- 面板服 ----------

#[tauri::command]
pub async fn panel_probe(kind: String, base: String, key: String) -> CmdResult<Vec<panel::PanelInst>> {
    if !["mcsm", "ptero"].contains(&kind.as_str()) {
        return Err("不认识的面板".into());
    }
    let p = panel::Panel { kind, base: base.trim().to_string(), key: key.trim().to_string() };
    p.list().await.map_err(err)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPanel {
    pub kind: String,
    pub base: String,
    pub key: String,
    pub panel_inst: String,
    pub panel_daemon: String,
    pub name: String,
    pub inst: String,
    pub address: String,
}

#[tauri::command]
pub fn panel_add(state: State<'_, AppState>, form: AddPanel) -> CmdResult<Value> {
    let (mc, loader, lv) = inst_info(&state, &form.inst).unwrap_or_default();
    let (host, port) = match form.address.trim().rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
        _ => (form.address.trim().to_string(), 25565),
    };
    let e = Entry {
        id: server::token(8).to_lowercase(),
        name: form.name,
        kind: form.kind,
        host,
        port,
        inst: form.inst,
        mc,
        loader,
        loader_version: lv,
        panel_base: form.base.trim().trim_end_matches('/').to_string(),
        panel_key: crate::auth::vault::seal(form.key.trim().as_bytes()).map_err(err)?,
        panel_inst: form.panel_inst,
        panel_daemon: form.panel_daemon,
        created: crate::base::util::now_ms(),
        ..Default::default()
    };
    server::put(&data(&state), &e).map_err(err)?;
    Ok(e.view())
}

/// 把本地实例的 Mod 传到面板服（只传服务端要的）
#[tauri::command]
pub async fn panel_sync_mods(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<u64> {
    let e = server::get(&data(&state), &id).map_err(err)?;
    let gd = local_game_dir(&state, &e.inst).ok_or("找不到本地实例")?;
    let p = e.panel().map_err(err)?;
    let task = state.tasks.create(format!("传 Mod 到「{}」", e.name), e.panel_base.clone(), String::new());
    let tid = task.snapshot().id;
    task::spawn_ticker(app.clone(), task.clone());
    tauri::async_runtime::spawn(async move {
        let r: anyhow::Result<Option<String>> = async {
            task.stage("挑出服务端要的 Mod");
            let mods = server::server_mods(&gd.join("mods"), &|l| task.log(l)).await?;
            for (k, (n, path, _)) in mods.iter().enumerate() {
                task.stage(format!("传 {n}（{}/{}）", k + 1, mods.len()));
                let bytes = tokio::fs::read(path).await?;
                p.upload(&e.panel_inst, &e.panel_daemon, "/mods", n, bytes).await.with_context(|| format!("传 {n} 失败"))?;
            }
            Ok(Some(format!("传了 {} 个 Mod，重启服务器生效（面板上多出来的旧 Mod 要自己删）", mods.len())))
        }
        .await;
        task::finish(&app, &task, r);
    });
    Ok(tid)
}
