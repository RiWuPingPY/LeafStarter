//! 账户：微软正版和离线。账户清单存在数据目录的 accounts.json，
//! 微软的刷新令牌用 vault 加密后才落盘；Minecraft 令牌只放内存，过期就用刷新令牌换新的。
//! 令牌永远不发给前端，也不进日志。

pub mod msa;
pub mod region;
pub mod vault;

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
    /// msa / offline
    kind: String,
    name: String,
    uuid: String,
    /// 加密后的微软刷新令牌（只有 msa 有）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    refresh: String,
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
}

/// 启动游戏要的身份
pub struct LaunchAuth {
    pub name: String,
    pub uuid: String,
    pub token: String,
    /// msa / legacy
    pub user_type: &'static str,
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
}

impl Auth {
    pub fn load(data_dir: &Path) -> Auth {
        let path = data_dir.join("accounts.json");
        let store = read_json(&path).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        Auth { path, store: Mutex::new(store), tokens: Mutex::new(HashMap::new()), mainland: tokio::sync::Mutex::new(None) }
    }

    fn save(&self) {
        let s = self.store.lock().unwrap();
        let _ = write_json(&self.path, &serde_json::to_value(&*s).unwrap_or_default());
    }

    pub fn views(&self) -> Vec<AccountView> {
        let s = self.store.lock().unwrap();
        let def = s.default.clone().or_else(|| s.accounts.first().map(|a| a.id.clone()));
        s.accounts.iter().map(|a| AccountView { id: a.id.clone(), kind: a.kind.clone(), name: a.name.clone(), uuid: a.uuid.clone(), default: Some(&a.id) == def.as_ref() }).collect()
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
            s.accounts.push(Account { id: id.clone(), kind: "offline".into(), name: name.into(), uuid: offline_uuid(name), refresh: String::new() });
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
            s.accounts.push(Account { id: id.clone(), kind: "msa".into(), name: mc.name.clone(), uuid: mc.uuid.clone(), refresh: sealed });
            s.msa_ever = true;
            if s.default.is_none() || !s.accounts.iter().any(|a| Some(&a.id) == s.default.as_ref()) {
                s.default = Some(id.clone());
            }
        }
        self.tokens.lock().unwrap().insert(id.clone(), (mc.access_token, now_ms() + mc.expires_in * 1000));
        self.save();
        Ok(self.views().into_iter().find(|v| v.id == id).unwrap())
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
            return Ok(LaunchAuth { name: acc.name, uuid: acc.uuid, token: "0".into(), user_type: "legacy" });
        }
        // 还有 10 分钟以上就直接用
        if let Some((tok, exp)) = self.tokens.lock().unwrap().get(&acc.id).cloned() {
            if exp > now_ms() + 600_000 {
                return Ok(LaunchAuth { name: acc.name, uuid: acc.uuid, token: tok, user_type: "msa" });
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
        Ok(LaunchAuth { name: mc.name, uuid: mc.uuid, token: mc.access_token, user_type: "msa" })
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
