//! 局域网直传的加密：ChaCha20-Poly1305。每个请求、回复的正文都是「12 字节随机数 + 密文」，
//! 附加数据是请求路径，换了路径解不开；解得开也就说明对方有这台设备的密钥。

use anyhow::{bail, Result};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chacha20poly1305::aead::rand_core::RngCore;
use sha2::{Digest, Sha256};

pub fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    OsRng.fill_bytes(&mut b);
    b
}

/// 从配对口令派生一次性的配对密钥
pub fn derive(label: &str, secret: &[u8]) -> [u8; 32] {
    Sha256::new().chain_update(label.as_bytes()).chain_update(secret).finalize().into()
}

pub fn seal(key: &[u8; 32], aad: &str, plain: &[u8]) -> Vec<u8> {
    let c = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut out = nonce.to_vec();
    out.extend(c.encrypt(&nonce, Payload { msg: plain, aad: aad.as_bytes() }).expect("加密不会失败"));
    out
}

pub fn open(key: &[u8; 32], aad: &str, data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 + 16 {
        bail!("数据不完整");
    }
    let c = ChaCha20Poly1305::new(Key::from_slice(key));
    c.decrypt(Nonce::from_slice(&data[..12]), Payload { msg: &data[12..], aad: aad.as_bytes() }).map_err(|_| anyhow::anyhow!("解不开：密钥不对或者数据被改过"))
}

// ---------- base64url（不补齐），配对码和密钥用 ----------

const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn b64(b: &[u8]) -> String {
    let mut out = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..=c.len() {
            out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
        }
    }
    out
}

pub fn unb64(s: &str) -> Result<Vec<u8>> {
    let mut out = vec![];
    let mut acc = 0u32;
    let mut bits = 0;
    for ch in s.trim().trim_end_matches('=').bytes() {
        let v = match T.iter().position(|&x| x == ch).or_else(|| match ch {
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }) {
            Some(v) => v as u32,
            None => bail!("配对码不对"),
        };
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

pub fn key32(s: &str) -> Result<[u8; 32]> {
    unb64(s)?.try_into().map_err(|_| anyhow::anyhow!("密钥长度不对"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open() {
        let k = random::<32>();
        let c = seal(&k, "/files", b"hello");
        assert_eq!(open(&k, "/files", &c).unwrap(), b"hello");
        assert!(open(&k, "/get", &c).is_err(), "路径不同解不开");
        assert!(open(&random::<32>(), "/files", &c).is_err(), "密钥不同解不开");
        let mut bad = c.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(open(&k, "/files", &bad).is_err(), "改过的解不开");
    }

    #[test]
    fn b64_round_trip() {
        for n in 0..40 {
            let b: Vec<u8> = (0..n).map(|x| (x * 37 + 11) as u8).collect();
            assert_eq!(unb64(&b64(&b)).unwrap(), b);
        }
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64(b"fo"), "Zm8");
    }
}
