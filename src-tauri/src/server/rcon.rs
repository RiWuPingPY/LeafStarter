//! RCON：给服务器发控制台命令（list、whitelist add、save-all……）。
//! 包格式：长度(i32 LE) + 请求 id + 类型（3 登录、2 命令、0 回复）+ 内容 + 两个 0。
//! 服务器上的 RCON 端口不对外开放，走 SSH 隧道；本机测试直接连 TCP。

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct Rcon<S> {
    s: S,
    id: i32,
}

async fn write_packet<S: AsyncWrite + Unpin>(s: &mut S, id: i32, kind: i32, body: &str) -> Result<()> {
    let b = body.as_bytes();
    let mut p = Vec::with_capacity(14 + b.len());
    p.extend_from_slice(&((10 + b.len()) as i32).to_le_bytes());
    p.extend_from_slice(&id.to_le_bytes());
    p.extend_from_slice(&kind.to_le_bytes());
    p.extend_from_slice(b);
    p.extend_from_slice(&[0, 0]);
    s.write_all(&p).await?;
    s.flush().await?;
    Ok(())
}

async fn read_packet<S: AsyncRead + Unpin>(s: &mut S) -> Result<(i32, i32, String)> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).await.context("RCON 断开了")?;
    let len = i32::from_le_bytes(n);
    if !(10..=1 << 20).contains(&len) {
        bail!("RCON 回复的长度不对：{len}");
    }
    let mut buf = vec![0u8; len as usize];
    s.read_exact(&mut buf).await?;
    let id = i32::from_le_bytes(buf[0..4].try_into().unwrap());
    let kind = i32::from_le_bytes(buf[4..8].try_into().unwrap());
    let body = String::from_utf8_lossy(&buf[8..buf.len() - 2]).into_owned();
    Ok((id, kind, body))
}

impl<S: AsyncRead + AsyncWrite + Unpin> Rcon<S> {
    pub async fn login(mut s: S, password: &str) -> Result<Rcon<S>> {
        write_packet(&mut s, 1, 3, password).await?;
        // 有的服务端先回一个空的回复包，再回登录结果
        for _ in 0..2 {
            let (id, kind, _) = tokio::time::timeout(std::time::Duration::from_secs(10), read_packet(&mut s)).await.context("RCON 没回应")??;
            if kind == 2 {
                if id == -1 {
                    bail!("RCON 密码不对");
                }
                return Ok(Rcon { s, id: 1 });
            }
        }
        bail!("RCON 登录没有结果")
    }

    /// 发一条命令，拿回输出。长输出会分好几个包：后面跟一个空命令当结束标记
    pub async fn cmd(&mut self, command: &str) -> Result<String> {
        self.id += 1;
        let id = self.id;
        write_packet(&mut self.s, id, 2, command).await?;
        self.id += 1;
        let end = self.id;
        write_packet(&mut self.s, end, 2, "").await?;
        let mut out = String::new();
        loop {
            let (rid, _, body) = tokio::time::timeout(std::time::Duration::from_secs(15), read_packet(&mut self.s)).await.context("RCON 没回应")??;
            if rid == end {
                break;
            }
            if rid == id {
                out.push_str(&body);
            }
        }
        Ok(strip_colors(&out))
    }
}

/// 去掉 §a 这类颜色代码
pub fn strip_colors(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
            continue;
        }
        if c == '§' {
            skip = true;
            continue;
        }
        out.push(c);
    }
    out
}

/// `list` 的输出：There are 2 of a max of 20 players online: a, b
pub fn parse_list(s: &str) -> Option<(u32, u32, Vec<String>)> {
    let nums: Vec<u32> = s.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).take(2).collect();
    if nums.len() < 2 {
        return None;
    }
    let names = s.split_once(':').map(|(_, r)| r.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    Some((nums[0], nums[1], names))
}

#[cfg(test)]
mod tests {
    #[test]
    fn list_and_colors() {
        assert_eq!(super::parse_list("There are 2 of a max of 20 players online: Steve, Alex"), Some((2, 20, vec!["Steve".into(), "Alex".into()])));
        assert_eq!(super::parse_list("There are 0 of a max of 10 players online: "), Some((0, 10, vec![])));
        assert_eq!(super::strip_colors("§aTPS: §f20.0"), "TPS: 20.0");
    }
}
