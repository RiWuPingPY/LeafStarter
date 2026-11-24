//! 配对过的设备。存在 `<数据目录>/sync/devices.json`，密钥用 vault 加密后才落盘。
//! - `clients`：可以连到我这里的设备（我是电脑、它是手机或另一台电脑）
//! - `peers`：我可以连过去的设备

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::crypto;
use crate::auth::vault;
use crate::base::util::now_ms;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Me {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Known {
    pub id: String,
    pub name: String,
    /// vault 加密过的 32 字节密钥
    pub key: String,
    pub added: u64,
    /// 连过去用（peers 才有）：对方的地址和端口
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default)]
    pub port: u16,
}

impl Known {
    pub fn key(&self) -> Result<[u8; 32]> {
        vault::open(&self.key)?.try_into().map_err(|_| anyhow::anyhow!("保存的密钥坏了，重新配对一次"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Devices {
    pub me: Me,
    #[serde(default)]
    pub clients: Vec<Known>,
    #[serde(default)]
    pub peers: Vec<Known>,
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("sync").join("devices.json")
}

impl Devices {
    pub fn load(data_dir: &Path) -> Devices {
        let mut d: Devices = std::fs::read(path(data_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if d.me.id.is_empty() {
            d.me.id = crypto::b64(&crypto::random::<9>());
            d.me.name = std::env::var("COMPUTERNAME").ok().filter(|n| !n.is_empty()).map(|n| format!("{n} 的电脑")).unwrap_or_else(|| "我的电脑".into());
            let _ = d.save(data_dir);
        }
        d
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let p = path(data_dir);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn new_known(id: &str, name: &str, key: &[u8; 32], hosts: Vec<String>, port: u16) -> Result<Known> {
        Ok(Known { id: id.into(), name: name.into(), key: vault::seal(key)?, added: now_ms(), hosts, port })
    }

    pub fn client(&self, id: &str) -> Option<&Known> {
        self.clients.iter().find(|c| c.id == id)
    }

    pub fn peer(&self, id: &str) -> Option<&Known> {
        self.peers.iter().find(|c| c.id == id)
    }
}
