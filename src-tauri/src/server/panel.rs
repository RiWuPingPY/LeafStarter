//! 面板服（ROADMAP 5.12 ⑥）：MCSManager（面板地址 + API Key）和翼龙 Pterodactyl（客户端 API Key）。
//! 能做的：列实例、启停、发命令、看状态和输出、上传 Mod。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct Panel {
    /// mcsm / ptero
    pub kind: String,
    pub base: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelInst {
    pub id: String,
    /// MCSManager 的节点 id；翼龙没有
    pub daemon: String,
    pub name: String,
    /// running / starting / stopping / stopped / unknown
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PanelStatus {
    pub state: String,
    pub memory_mb: Option<u64>,
    pub cpu: Option<f64>,
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(30)).build()?)
}

fn mcsm_state(n: i64) -> &'static str {
    match n {
        3 => "running",
        2 => "starting",
        1 => "stopping",
        0 => "stopped",
        _ => "unknown",
    }
}

impl Panel {
    fn base(&self) -> &str {
        self.base.trim().trim_end_matches('/')
    }

    async fn mcsm(&self, method: reqwest::Method, path: &str, query: &[(&str, &str)], body: Option<Value>) -> Result<Value> {
        let mut q: Vec<(&str, &str)> = query.to_vec();
        q.push(("apikey", &self.key));
        let mut req = http()?.request(method, format!("{}{path}", self.base())).query(&q).header("X-Requested-With", "XMLHttpRequest");
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.context("连不上 MCSManager 面板")?;
        let status = r.status();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        if !status.is_success() || v["status"].as_i64().is_some_and(|s| s != 200) {
            let msg = v["data"].as_str().or(v["message"].as_str()).unwrap_or("");
            bail!("MCSManager 返回 {}：{}", status.as_u16(), if status.as_u16() == 403 { "API Key 不对或者没有权限" } else { msg });
        }
        Ok(v["data"].clone())
    }

    async fn ptero(&self, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
        let mut req = http()?.request(method, format!("{}/api/client{path}", self.base())).bearer_auth(&self.key).header("Accept", "Application/vnd.pterodactyl.v1+json");
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.context("连不上翼龙面板")?;
        let status = r.status();
        let text = r.text().await.unwrap_or_default();
        if !status.is_success() {
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let msg = v["errors"][0]["detail"].as_str().unwrap_or("");
            bail!("翼龙面板返回 {}：{}", status.as_u16(), if status.as_u16() == 401 || status.as_u16() == 403 { "API Key 不对（要用「账户 → API 凭据」里的客户端 Key）" } else { msg });
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// 这个 Key 能管的实例
    pub async fn list(&self) -> Result<Vec<PanelInst>> {
        if self.kind == "ptero" {
            let v = self.ptero(reqwest::Method::GET, "", None).await?;
            return Ok(v["data"]
                .as_array()
                .map(|a| a.iter().map(|s| PanelInst { id: s["attributes"]["identifier"].as_str().unwrap_or("").into(), daemon: String::new(), name: s["attributes"]["name"].as_str().unwrap_or("").into(), state: "unknown".into() }).collect())
                .unwrap_or_default());
        }
        // 普通用户：自己的实例列表；管理员：逐个节点列
        let me = self.mcsm(reqwest::Method::GET, "/api/auth/", &[("advanced", "true")], None).await?;
        let mut out: Vec<PanelInst> = me["instances"]
            .as_array()
            .map(|a| a.iter().map(|i| PanelInst { id: i["instanceUuid"].as_str().unwrap_or("").into(), daemon: i["daemonId"].as_str().unwrap_or("").into(), name: i["nickname"].as_str().unwrap_or("").into(), state: mcsm_state(i["status"].as_i64().unwrap_or(-1)).into() }).collect())
            .unwrap_or_default();
        if out.is_empty() && me["permission"].as_i64().unwrap_or(0) >= 10 {
            let nodes = self.mcsm(reqwest::Method::GET, "/api/service/remote_services_list", &[], None).await?;
            for n in nodes.as_array().cloned().unwrap_or_default() {
                let d = n["uuid"].as_str().unwrap_or("").to_string();
                let r = self.mcsm(reqwest::Method::GET, "/api/service/remote_service_instances", &[("daemonId", &d), ("page", "1"), ("page_size", "100"), ("instance_name", ""), ("status", "")], None).await;
                if let Ok(r) = r {
                    for i in r["data"].as_array().cloned().unwrap_or_default() {
                        out.push(PanelInst { id: i["instanceUuid"].as_str().unwrap_or("").into(), daemon: d.clone(), name: i["config"]["nickname"].as_str().unwrap_or("").into(), state: mcsm_state(i["status"].as_i64().unwrap_or(-1)).into() });
                    }
                }
            }
        }
        Ok(out)
    }

    /// start / stop / restart / kill
    pub async fn power(&self, inst: &str, daemon: &str, action: &str) -> Result<()> {
        if !["start", "stop", "restart", "kill"].contains(&action) {
            bail!("不认识的操作");
        }
        if self.kind == "ptero" {
            self.ptero(reqwest::Method::POST, &format!("/servers/{inst}/power"), Some(json!({ "signal": action }))).await?;
        } else {
            let path = if action == "start" { "open" } else { action };
            self.mcsm(reqwest::Method::GET, &format!("/api/protected_instance/{path}"), &[("uuid", inst), ("daemonId", daemon)], None).await?;
        }
        Ok(())
    }

    pub async fn command(&self, inst: &str, daemon: &str, cmd: &str) -> Result<()> {
        if self.kind == "ptero" {
            self.ptero(reqwest::Method::POST, &format!("/servers/{inst}/command"), Some(json!({ "command": cmd }))).await.map_err(|e| if e.to_string().contains("502") { anyhow::anyhow!("服务器没在运行") } else { e })?;
        } else {
            self.mcsm(reqwest::Method::GET, "/api/protected_instance/command", &[("uuid", inst), ("daemonId", daemon), ("command", cmd)], None).await?;
        }
        Ok(())
    }

    /// 最近的控制台输出（翼龙只能走 WebSocket，这里不做，返回空）
    pub async fn log(&self, inst: &str, daemon: &str) -> Result<String> {
        if self.kind == "ptero" {
            return Ok(String::new());
        }
        let v = self.mcsm(reqwest::Method::GET, "/api/protected_instance/outputlog", &[("uuid", inst), ("daemonId", daemon)], None).await?;
        Ok(strip_ansi(v.as_str().unwrap_or("")))
    }

    pub async fn status(&self, inst: &str, daemon: &str) -> Result<PanelStatus> {
        if self.kind == "ptero" {
            let v = self.ptero(reqwest::Method::GET, &format!("/servers/{inst}/resources"), None).await?;
            let a = &v["attributes"];
            let st = match a["current_state"].as_str().unwrap_or("") {
                "running" => "running",
                "starting" => "starting",
                "stopping" => "stopping",
                "offline" => "stopped",
                _ => "unknown",
            };
            return Ok(PanelStatus { state: st.into(), memory_mb: a["resources"]["memory_bytes"].as_u64().map(|b| b / 1048576), cpu: a["resources"]["cpu_absolute"].as_f64() });
        }
        let v = self.mcsm(reqwest::Method::GET, "/api/instance", &[("uuid", inst), ("daemonId", daemon)], None).await?;
        Ok(PanelStatus { state: mcsm_state(v["status"].as_i64().unwrap_or(-1)).into(), memory_mb: v["processInfo"]["memory"].as_u64().map(|b| b / 1048576), cpu: v["processInfo"]["cpu"].as_f64() })
    }

    /// 传一个文件到实例的某个目录（比如 /mods）
    pub async fn upload(&self, inst: &str, daemon: &str, dir: &str, name: &str, bytes: Vec<u8>) -> Result<()> {
        let part = reqwest::multipart::Part::bytes(bytes).file_name(name.to_string());
        if self.kind == "ptero" {
            let v = self.ptero(reqwest::Method::GET, &format!("/servers/{inst}/files/upload"), None).await?;
            let url = v["attributes"]["url"].as_str().context("面板没给上传地址")?;
            let r = http()?.post(format!("{url}&directory={}", urlencode(dir))).multipart(reqwest::multipart::Form::new().part("files", part)).send().await?;
            if !r.status().is_success() {
                bail!("上传失败（{}）", r.status());
            }
            return Ok(());
        }
        let v = self.mcsm(reqwest::Method::POST, "/api/files/upload", &[("uuid", inst), ("daemonId", daemon), ("upload_dir", dir)], None).await?;
        let (pw, addr) = (v["password"].as_str().context("面板没给上传口令")?, v["addr"].as_str().context("面板没给节点地址")?);
        let proto = if self.base().starts_with("https") { "https" } else { "http" };
        let addr = if addr.starts_with("http") { addr.to_string() } else { format!("{proto}://{addr}") };
        let r = http()?.post(format!("{addr}/upload/{pw}")).multipart(reqwest::multipart::Form::new().part("file", part)).send().await.context("连不上面板的节点（节点地址可能是内网地址）")?;
        if !r.status().is_success() {
            bail!("上传失败（{}）", r.status());
        }
        Ok(())
    }
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// 控制台输出里的颜色控制码
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn ansi() {
        assert_eq!(super::strip_ansi("\u{1b}[32mDone\u{1b}[0m (1.2s)"), "Done (1.2s)");
        assert_eq!(super::urlencode("/mods 1"), "/mods%201");
    }
}
