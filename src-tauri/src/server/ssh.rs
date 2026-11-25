//! SSH（M10）：连常驻服务器。
//! - 密码只用一次：登录后装上我们生成的 ed25519 公钥，以后都用密钥；密码不保存
//! - 第一次连上记下服务器的指纹（SHA256），之后对不上就拒绝连（可能被人冒充了）
//! - 传文件不靠 SFTP：`cat > 文件` 往里灌、`cat 文件` 往外读，服务器上不用装别的

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use russh::client;
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::keys::ssh_key::LineEnding;
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg};
use russh::ChannelMsg;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub enum Auth {
    Password(String),
    /// OpenSSH 或 PEM 格式的私钥
    Key(String),
}

struct Hk {
    expect: Option<String>,
    seen: Arc<Mutex<String>>,
}

impl client::Handler for Hk {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &russh::keys::PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let fp = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        *self.seen.lock().unwrap() = fp.clone();
        Ok(self.expect.as_deref().is_none_or(|e| e == fp))
    }
}

pub struct Conn {
    h: client::Handle<Hk>,
    /// 服务器的指纹
    pub fingerprint: String,
}

/// 连上并登录。`expect` 是上次记下的指纹，第一次连传 None
pub async fn connect(host: &str, port: u16, user: &str, auth: &Auth, expect: Option<&str>) -> Result<Conn> {
    let cfg = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(900)),
        keepalive_interval: Some(Duration::from_secs(20)),
        ..Default::default()
    });
    let seen = Arc::new(Mutex::new(String::new()));
    let hk = Hk { expect: expect.map(String::from), seen: seen.clone() };
    let h = tokio::time::timeout(Duration::from_secs(20), client::connect(cfg, (host, port), hk)).await.with_context(|| format!("连 {host}:{port} 超时了，检查 IP、端口和云防火墙"))?;
    let mut h = match h {
        Ok(h) => h,
        Err(russh::Error::UnknownKey) => bail!("服务器的身份和上次不一样（指纹变了）。如果你重装过系统，在服务器设置里点「重新信任」；不然可能有人在冒充它，先别连"),
        Err(e) => return Err(e).with_context(|| format!("连不上 {host}:{port}")),
    };
    let ok = match auth {
        Auth::Password(p) => h.authenticate_password(user, p).await.context("登录时出错")?.success(),
        Auth::Key(k) => {
            let key = russh::keys::decode_secret_key(k, None).context("密钥读不了（有密码保护的密钥暂时不支持）")?;
            let hash = h.best_supported_rsa_hash().await.ok().flatten().flatten();
            h.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await.context("登录时出错")?.success()
        }
    };
    if !ok {
        bail!(match auth {
            Auth::Password(_) => "用户名或密码不对",
            Auth::Key(_) => "密钥登录被拒绝了（服务器上可能没装这把公钥）",
        });
    }
    let fingerprint = seen.lock().unwrap().clone();
    Ok(Conn { h, fingerprint })
}

/// 单引号转义，给 shell 用
pub fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// 服务器上的路径：开头的 `$HOME/` 要让 shell 展开，其余部分原样（单引号）
pub fn rq(p: &str) -> String {
    match p.strip_prefix("$HOME/") {
        Some(rest) => format!("\"$HOME\"/{}", shq(rest)),
        None => shq(p),
    }
}

pub struct Output {
    pub code: u32,
    /// stdout 和 stderr 合在一起，最多留最后 64 KB
    pub text: String,
}

impl Conn {
    /// 执行一条命令，stdin 可以灌数据；每一行输出回调一次
    pub async fn exec_with<R: AsyncRead + Unpin>(&self, cmd: &str, stdin: Option<R>, mut on_line: impl FnMut(&str)) -> Result<Output> {
        let mut ch = self.h.channel_open_session().await.context("开不了 SSH 通道")?;
        ch.exec(true, cmd).await?;
        if let Some(mut r) = stdin {
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                let n = r.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                ch.data(&buf[..n]).await.context("往服务器写数据时断了")?;
            }
            ch.eof().await?;
        }
        let mut text = String::new();
        let mut line = Vec::new();
        let mut code = None;
        let mut push = |bytes: &[u8], text: &mut String, line: &mut Vec<u8>| {
            for &b in bytes {
                if b == b'\n' || b == b'\r' {
                    if !line.is_empty() {
                        let l = String::from_utf8_lossy(line).into_owned();
                        on_line(&l);
                        text.push_str(&l);
                        text.push('\n');
                        line.clear();
                    }
                } else {
                    line.push(b);
                }
            }
            if text.len() > 64 * 1024 {
                let cut = text.len() - 48 * 1024;
                let cut = (cut..text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(cut);
                text.drain(..cut);
            }
        };
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => push(data, &mut text, &mut line),
                ChannelMsg::ExtendedData { ref data, .. } => push(data, &mut text, &mut line),
                ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
                _ => {}
            }
        }
        if !line.is_empty() {
            let l = String::from_utf8_lossy(&line).into_owned();
            on_line(&l);
            text.push_str(&l);
        }
        Ok(Output { code: code.unwrap_or(255), text })
    }

    pub async fn exec(&self, cmd: &str) -> Result<Output> {
        self.exec_with(cmd, None::<&[u8]>, |_| {}).await
    }

    /// 执行，退出码不是 0 就报错（带最后几行输出）
    pub async fn run(&self, cmd: &str) -> Result<String> {
        let o = self.exec(cmd).await?;
        if o.code != 0 {
            bail!("命令失败（退出码 {}）：{}", o.code, tail(&o.text, 8));
        }
        Ok(o.text)
    }

    /// 用 bash 跑一段脚本（从 stdin 送进去，不受命令行长度限制）
    pub async fn script(&self, body: &str, on_line: impl FnMut(&str)) -> Result<Output> {
        let full = format!("set -e\nexport LC_ALL=C DEBIAN_FRONTEND=noninteractive\n{body}\n");
        self.exec_with("bash -s", Some(full.as_bytes()), on_line).await
    }

    /// 把数据写成服务器上的一个文件（先写临时文件再改名）
    pub async fn upload<R: AsyncRead + Unpin>(&self, data: R, remote: &str) -> Result<()> {
        let tmp = format!("{remote}.ls-part");
        let cmd = format!("mkdir -p \"$(dirname {q})\" && cat > {t} && mv -f {t} {q}", q = rq(remote), t = rq(&tmp));
        let o = self.exec_with(&cmd, Some(data), |_| {}).await?;
        if o.code != 0 {
            bail!("传不上去：{}", tail(&o.text, 4));
        }
        Ok(())
    }

    /// 执行一条命令，把它的标准输出写进本地（下载文件、打包世界）
    pub async fn download_cmd<W: AsyncWrite + Unpin>(&self, cmd: &str, out: &mut W, on_bytes: impl Fn(u64)) -> Result<u64> {
        let mut ch = self.h.channel_open_session().await?;
        ch.exec(true, cmd).await?;
        let mut n = 0u64;
        let mut err = String::new();
        let mut code = None;
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => {
                    out.write_all(data).await?;
                    n += data.len() as u64;
                    on_bytes(n);
                }
                ChannelMsg::ExtendedData { ref data, .. } => {
                    if err.len() < 4096 {
                        err.push_str(&String::from_utf8_lossy(data));
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
                _ => {}
            }
        }
        out.flush().await?;
        if code != Some(0) {
            bail!("下载失败：{}", tail(&err, 4));
        }
        Ok(n)
    }

    /// 经 SSH 隧道连服务器本机的一个端口（RCON 不对外开放，走这里）
    pub async fn tunnel(&self, port: u16) -> Result<russh::ChannelStream<client::Msg>> {
        let ch = self.h.channel_open_direct_tcpip("127.0.0.1", port as u32, "127.0.0.1", 0).await.context("开不了 SSH 隧道")?;
        Ok(ch.into_stream())
    }

    pub async fn close(self) {
        let _ = self.h.disconnect(russh::Disconnect::ByApplication, "", "zh").await;
    }
}

pub fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// 生成一把 ed25519 密钥：(OpenSSH 私钥, authorized_keys 里那一行)
pub fn gen_key(comment: &str) -> Result<(String, String)> {
    let seed = crate::sync::crypto::random::<32>();
    let kp = Ed25519Keypair::from_seed(&seed);
    let key = PrivateKey::new(KeypairData::Ed25519(kp), comment)?;
    let private = key.to_openssh(LineEnding::LF)?.to_string();
    let public = key.public_key().to_openssh()?;
    Ok((private, public))
}

#[cfg(test)]
mod tests {
    #[test]
    fn keys_and_quote() {
        let (p, pubk) = super::gen_key("leafstarter").unwrap();
        assert!(p.contains("BEGIN OPENSSH PRIVATE KEY"));
        assert!(pubk.starts_with("ssh-ed25519 ") && pubk.ends_with("leafstarter"));
        assert!(russh::keys::decode_secret_key(&p, None).is_ok());
        assert_eq!(super::shq("a'b"), r"'a'\''b'");
        assert_eq!(super::rq("$HOME/x/a b.jar"), r#""$HOME"/'x/a b.jar'"#);
    }
}
