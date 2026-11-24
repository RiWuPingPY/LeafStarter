//! 账户：微软正版、离线、外置登录（authlib-injector）。账户清单存在数据目录的 accounts.json，
//! 微软的刷新令牌、外置登录的令牌用 vault 加密后才落盘；Minecraft 令牌只放内存，过期就用刷新令牌换新的。
//! 令牌永远不发给前端，也不进日志。

pub mod msa;
pub mod region;
pub mod vault;
pub mod ygg;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::base::util::{now_ms, read_json, write_json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    id: String,
    /// msa / offline / ygg
    kind: String,
    name: String,
    uuid: String,
    /// 加密后的令牌：微软是刷新令牌，外置登录是 accessToken
    #[serde(default, skip_serializing_if = "String::is_empty")]
    refresh: String,
    /// 外置登录：API 地址、服务器名字、clientToken
    #[serde(default, skip_serializing_if = "String::is_empty")]
    server: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    server_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    client: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Store {
    #[serde(default)]
    default: Option<String>,
    /// 这台设备上登录成功过正版（D6：大陆以外要先有过才能加离线账户）
    #[serde(default)]
    msa_ever: bool,
    #[serde(default)]
    accounts: Vec<Account>,
}

/// 给前端看的账户（没有任何令牌）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uuid: String,
    pub default: bool,
    /// 外置登录的服务器名字
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

/// 外置登录的一步：登好了，或者账号下有好几个角色要选
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "step")]
pub enum YggStep {
    Done { account: AccountView },
    Choose { ticket: String, server: String, profiles: Vec<ygg::Profile> },
}

/// 启动游戏要的身份
pub struct LaunchAuth {
    pub name: String,
    pub uuid: String,
    pub token: String,
    /// msa / legacy / mojang（外置登录）
    pub user_type: &'static str,
    /// 外置登录：(API 地址, 预取的元数据)；启动时要加 authlib-injector
    pub ygg: Option<(String, String)>,
}

/// 和 Java 版一致的离线 UUID：对「OfflinePlayer:名字」做 MD5，按 UUID v3 设版本位
pub fn offline_uuid(name: &str) -> String {
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{}", name).as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 游戏名：3–16 个字符，只能是英文字母、数字和下划线（不然进服务器、存档都可能出问题）
pub fn check_offline_name(name: &str) -> Result<()> {
    let n = name.chars().count();
    if !(3..=16).contains(&n) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("游戏名要 3 到 16 个字符，只能用英文字母、数字和下划线");
    }
    Ok(())
}

pub struct Auth {
    path: PathBuf,
    store: Mutex<Store>,
    /// 账户 id → (Minecraft 令牌, 过期时间毫秒)
    tokens: Mutex<HashMap<String, (String, u64)>>,
    /// 这次运行查过的地区结果
    mainland: tokio::sync::Mutex<Option<bool>>,
    /// 外置登录选角色前暂存的会话：ticket → (服务器, 会话)
    pending: Mutex<HashMap<String, (ygg::Server, ygg::Session)>>,
}

impl Auth {
    pub fn load(data_dir: &Path) -> Auth {
        let path = data_dir.join("accounts.json");
        let store = read_json(&path).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        Auth { path, store: Mutex::new(store), tokens: Mutex::new(HashMap::new()), mainland: tokio::sync::Mutex::new(None), pending: Mutex::new(HashMap::new()) }
    }

    fn save(&self) {
        let s = self.store.lock().unwrap();
        let _ = write_json(&self.path, &serde_json::to_value(&*s).unwrap_or_default());
    }

    pub fn views(&self) -> Vec<AccountView> {
        let s = self.store.lock().unwrap();
        let def = s.default.clone().or_else(|| s.accounts.first().map(|a| a.id.clone()));
        s.accounts
            .iter()
            .map(|a| AccountView {
                id: a.id.clone(),
                kind: a.kind.clone(),
                name: a.name.clone(),
                uuid: a.uuid.clone(),
                default: Some(&a.id) == def.as_ref(),
                server: (a.kind == "ygg").then(|| a.server_name.clone()),
            })
            .collect()
    }

    /// D6：能不能加离线账户，不能的话说明原因
    pub async fn offline_allowed(&self) -> (bool, &'static str) {
        if self.store.lock().unwrap().msa_ever {
            return (true, "");
        }
        let mut cached = self.mainland.lock().await;
        if cached.is_none() {
            *cached = Some(region::in_mainland().await);
        }
        if cached.unwrap() {
            (true, "")
        } else {
            (false, "在中国大陆以外，要先在这台电脑上登录过一次微软正版账户，才能添加离线账户")
        }
    }

    pub async fn add_offline(&self, name: &str) -> Result<AccountView> {
        let name = name.trim();
        check_offline_name(name)?;
        let (ok, why) = self.offline_allowed().await;
        if !ok {
            bail!("{}", why);
        }
        let id = format!("offline:{}", name.to_ascii_lowercase());
        {
            let mut s = self.store.lock().unwrap();
            if s.accounts.iter().any(|a| a.id == id) {
                bail!("已经有叫 {} 的离线账户了", name);
            }
            s.accounts.push(Account { id: id.clone(), kind: "offline".into(), name: name.into(), uuid: offline_uuid(name), refresh: String::new(), server: String::new(), server_name: String::new(), client: String::new() });
            if s.default.is_none() {
                s.default = Some(id.clone());
            }
        }
        self.save();
        Ok(self.views().into_iter().find(|v| v.id == id).unwrap())
    }

    /// 设备代码登录成功后：查档案、存账户
    pub async fn finish_msa(&self, http: &reqwest::Client, token: msa::MsToken) -> Result<AccountView> {
        let mc = msa::minecraft(http, &token.access).await?;
        let id = format!("msa:{}", mc.uuid);
        let sealed = vault::seal(token.refresh.as_bytes())?;
        {
            let mut s = self.store.lock().unwrap();
            s.accounts.retain(|a| a.id != id);
            s.accounts.push(Account { id: id.clone(), kind: "msa".into(), name: mc.name.clone(), uuid: mc.uuid.clone(), refresh: sealed, server: String::new(), server_name: String::new(), client: String::new() });
            s.msa_ever = true;
            if s.default.is_none() || !s.accounts.iter().any(|a| Some(&a.id) == s.default.as_ref()) {
                s.default = Some(id.clone());
            }
        }
        self.tokens.lock().unwrap().insert(id.clone(), (mc.access_token, now_ms() + mc.expires_in * 1000));
        self.save();
        Ok(self.views().into_iter().find(|v| v.id == id).unwrap())
    }

    /// 外置登录：找服务器、用账号密码换令牌。只有一个角色就直接存好；好几个就先暂存，等选
    pub async fn ygg_login(&self, server: &str, username: &str, password: &str) -> Result<YggStep> {
        let http = ygg::client()?;
        let srv = ygg::discover(&http, server).await?;
        let s = ygg::authenticate(&http, &srv.api, username, password).await?;
        match s.selected.clone() {
            Some(p) => self.finish_ygg(srv, &s.access, &s.client, p),
            None if s.profiles.len() == 1 => {
                let p = s.profiles[0].clone();
                let r = ygg::refresh(&http, &srv.api, &s.access, &s.client, Some(&p)).await?;
                self.finish_ygg(srv, &r.access, &r.client, p)
            }
            None => {
                let ticket = format!("{:x}", now_ms());
                let (name, profiles) = (srv.name.clone(), s.profiles.clone());
                self.pending.lock().unwrap().insert(ticket.clone(), (srv, s));
                Ok(YggStep::Choose { ticket, server: name, profiles })
            }
        }
    }

    /// 外置登录：选好角色
    pub async fn ygg_pick(&self, ticket: &str, profile: &str) -> Result<YggStep> {
        let (srv, s) = self.pending.lock().unwrap().remove(ticket).context("登录过期了，重新登录一次")?;
        let p = s.profiles.iter().find(|p| p.id == profile).cloned().context("没有这个角色")?;
        let r = ygg::refresh(&ygg::client()?, &srv.api, &s.access, &s.client, Some(&p)).await?;
        self.finish_ygg(srv, &r.access, &r.client, p)
    }

    fn finish_ygg(&self, srv: ygg::Server, access: &str, client: &str, p: ygg::Profile) -> Result<YggStep> {
        let host = reqwest::Url::parse(&srv.api).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
        let id = format!("ygg:{}:{}", host, p.id);
        let sealed = vault::seal(access.as_bytes())?;
        {
            let mut s = self.store.lock().unwrap();
            s.accounts.retain(|a| a.id != id);
            s.accounts.push(Account { id: id.clone(), kind: "ygg".into(), name: p.name, uuid: p.id, refresh: sealed, server: srv.api, server_name: srv.name, client: client.into() });
            if s.default.is_none() {
                s.default = Some(id.clone());
            }
        }
        self.save();
        Ok(YggStep::Done { account: self.views().into_iter().find(|v| v.id == id).unwrap() })
    }

    pub fn remove(&self, id: &str) {
        {
            let mut s = self.store.lock().unwrap();
            s.accounts.retain(|a| a.id != id);
            if s.default.as_deref() == Some(id) {
                s.default = s.accounts.first().map(|a| a.id.clone());
            }
        }
        self.tokens.lock().unwrap().remove(id);
        self.save();
    }

    pub fn set_default(&self, id: &str) -> Result<()> {
        {
            let mut s = self.store.lock().unwrap();
            if !s.accounts.iter().any(|a| a.id == id) {
                bail!("找不到这个账户");
            }
            s.default = Some(id.to_string());
        }
        self.save();
        Ok(())
    }

    /// 启动前拿身份：没指定就用默认账户；微软账户的令牌快过期了就换新的
    pub async fn for_launch(&self, id: Option<&str>) -> Result<LaunchAuth> {
        let acc = {
            let s = self.store.lock().unwrap();
            let want = id.map(String::from).or_else(|| s.default.clone());
            want.and_then(|w| s.accounts.iter().find(|a| a.id == w).cloned()).or_else(|| s.accounts.first().cloned())
        };
        let Some(acc) = acc else { bail!("还没有账户。点左下角添加一个微软账户或离线账户") };
        if acc.kind == "offline" {
            return Ok(LaunchAuth { name: acc.name, uuid: acc.uuid, token: "0".into(), user_type: "legacy", ygg: None });
        }
        if acc.kind == "ygg" {
            return self.ygg_launch(acc).await;
        }
        // 还有 10 分钟以上就直接用
        if let Some((tok, exp)) = self.tokens.lock().unwrap().get(&acc.id).cloned() {
            if exp > now_ms() + 600_000 {
                return Ok(LaunchAuth { name: acc.name, uuid: acc.uuid, token: tok, user_type: "msa", ygg: None });
            }
        }
        let refresh = String::from_utf8(vault::open(&acc.refresh)?).context("保存的令牌坏了，重新登录一次")?;
        let http = msa::client()?;
        let ms = msa::refresh(&http, &refresh).await?;
        let mc = msa::minecraft(&http, &ms.access).await?;
        // 微软会轮换刷新令牌，存新的
        if !ms.refresh.is_empty() {
            let sealed = vault::seal(ms.refresh.as_bytes())?;
            let mut s = self.store.lock().unwrap();
            if let Some(a) = s.accounts.iter_mut().find(|a| a.id == acc.id) {
                a.refresh = sealed;
                a.name = mc.name.clone();
            }
        }
        self.save();
        self.tokens.lock().unwrap().insert(acc.id.clone(), (mc.access_token.clone(), now_ms() + mc.expires_in * 1000));
        Ok(LaunchAuth { name: mc.name, uuid: mc.uuid, token: mc.access_token, user_type: "msa", ygg: None })
    }

    /// 外置登录启动前：令牌还能用就用，不能用就刷新；顺便取一份服务器元数据
    async fn ygg_launch(&self, acc: Account) -> Result<LaunchAuth> {
        let http = ygg::client()?;
        let mut access = String::from_utf8(vault::open(&acc.refresh)?).context("保存的令牌坏了，重新登录一次")?;
        if !ygg::validate(&http, &acc.server, &access, &acc.client).await {
            let r = ygg::refresh(&http, &acc.server, &access, &acc.client, None).await.map_err(|e| anyhow::anyhow!("{}：{:#}。删掉这个账户重新登录一次", acc.server_name, e))?;
            access = r.access;
            let sealed = vault::seal(access.as_bytes())?;
            if let Some(a) = self.store.lock().unwrap().accounts.iter_mut().find(|a| a.id == acc.id) {
                a.refresh = sealed;
                if let Some(p) = r.selected {
                    a.name = p.name;
                }
            }
            self.save();
        }
        let meta = http.get(&acc.server).send().await.context("连不上外置登录的服务器")?.text().await?;
        let name = self.store.lock().unwrap().accounts.iter().find(|a| a.id == acc.id).map(|a| a.name.clone()).unwrap_or(acc.name);
        Ok(LaunchAuth { name, uuid: acc.uuid, token: access, user_type: "mojang", ygg: Some((acc.server, meta)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java() {
        // Java：UUID.nameUUIDFromBytes("OfflinePlayer:Steve".getBytes(UTF_8))
        assert_eq!(offline_uuid("Steve"), "5627dd98e6be3c21b8a8e92344183641");
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }

    #[test]
    fn checks_names() {
        assert!(check_offline_name("Steve_01").is_ok());
        assert!(check_offline_name("ab").is_err());
        assert!(check_offline_name("名字").is_err());
        assert!(check_offline_name("a".repeat(17).as_str()).is_err());
    }
}
