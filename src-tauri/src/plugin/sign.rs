//! 签名（定制包、插件包共用）：ed25519。
//! 包是一个 zip；`signature.json` 里放公钥和签名，签的是「其余每个文件名 + SHA-256」排好序拼起来的摘要。

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const SIG_FILE: &str = "signature.json";

pub fn b64(b: &[u8]) -> String {
    crate::base::util::base64(b)
}

pub fn unb64(s: &str) -> Option<Vec<u8>> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = vec![];
    let (mut buf, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = T.iter().position(|&x| x == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// 新的一把密钥（PKCS#8）
pub fn new_key() -> Result<Vec<u8>> {
    let rng = ring::rand::SystemRandom::new();
    Ok(Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| anyhow::anyhow!("生成密钥失败"))?.as_ref().to_vec())
}

pub fn public_of(pkcs8: &[u8]) -> Result<Vec<u8>> {
    Ok(Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| anyhow::anyhow!("密钥坏了"))?.public_key().as_ref().to_vec())
}

pub fn sign(pkcs8: &[u8], msg: &[u8]) -> Result<Vec<u8>> {
    Ok(Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| anyhow::anyhow!("密钥坏了"))?.sign(msg).as_ref().to_vec())
}

pub fn verify(public: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    UnparsedPublicKey::new(&ED25519, public).verify(msg, sig).is_ok()
}

/// 公钥的指纹：SHA-256 前 8 字节，四个一组
pub fn fingerprint(public: &[u8]) -> String {
    let h = Sha256::digest(public);
    h[..8].chunks(2).map(|c| format!("{:02X}{:02X}", c[0], c[1])).collect::<Vec<_>>().join("-")
}

/// 文件名 → SHA-256 的摘要（签名签的就是它）
pub fn digest(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut h = Sha256::new();
    for (name, data) in files {
        if name == SIG_FILE {
            continue;
        }
        h.update(name.as_bytes());
        h.update([0]);
        h.update(format!("{:x}", Sha256::digest(data)).as_bytes());
        h.update(b"\n");
    }
    h.finalize().to_vec()
}

/// 读一个包：所有文件（只认普通文件，路径不许往外跳），每个不超过 200 MB
pub fn read_zip(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let f = std::fs::File::open(path).with_context(|| format!("打不开 {}", path.display()))?;
    let mut z = zip::ZipArchive::new(f).context("不是有效的包（应该是 zip）")?;
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().replace('\\', "/");
        if name.starts_with('/') || name.split('/').any(|c| c == ".." || c.is_empty()) {
            bail!("包里有不安全的路径：{name}");
        }
        if e.size() > 200 * 1024 * 1024 {
            bail!("包里的 {name} 太大了");
        }
        let mut v = Vec::with_capacity(e.size() as usize);
        e.read_to_end(&mut v)?;
        out.insert(name, v);
    }
    Ok(out)
}

/// 校验签名，返回公钥
pub fn check(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let s = files.get(SIG_FILE).context("包没有签名")?;
    let v: Value = serde_json::from_slice(s).context("签名文件坏了")?;
    let key = v["key"].as_str().and_then(unb64).context("签名里没有公钥")?;
    let sig = v["sig"].as_str().and_then(unb64).context("签名里没有签名")?;
    if !verify(&key, &digest(files), &sig) {
        bail!("签名对不上：包被改过，或者不是作者发的");
    }
    Ok(key)
}

/// 签好名写成 zip
pub fn write_zip(path: &Path, files: &BTreeMap<String, Vec<u8>>, pkcs8: &[u8]) -> Result<Vec<u8>> {
    let public = public_of(pkcs8)?;
    let sig = sign(pkcs8, &digest(files))?;
    let sigjson = serde_json::to_vec_pretty(&json!({ "format": "leafstarter-sig/1", "alg": "ed25519", "key": b64(&public), "sig": b64(&sig) }))?;
    let tmp = path.with_extension("part");
    {
        let f = std::fs::File::create(&tmp)?;
        let mut z = zip::ZipWriter::new(f);
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in files.iter().filter(|(n, _)| *n != SIG_FILE) {
            z.start_file(name.as_str(), opt)?;
            z.write_all(data)?;
        }
        z.start_file(SIG_FILE, opt)?;
        z.write_all(&sigjson)?;
        z.finish()?;
    }
    std::fs::rename(&tmp, path).or_else(|_| crate::base::util::move_file(&tmp, path))?;
    Ok(public)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_roundtrip() {
        let k = new_key().unwrap();
        let mut files = BTreeMap::new();
        files.insert("a.json".to_string(), b"{}".to_vec());
        files.insert("icon.png".to_string(), vec![1, 2, 3]);
        let d = std::env::temp_dir().join(format!("ls-sign-{}.zip", std::process::id()));
        let public = write_zip(&d, &files, &k).unwrap();
        let back = read_zip(&d).unwrap();
        assert_eq!(check(&back).unwrap(), public);
        // 改一个字节就对不上
        let mut bad = back.clone();
        bad.insert("a.json".into(), b"{ }".to_vec());
        assert!(check(&bad).is_err());
        assert_eq!(fingerprint(&public).len(), 19);
        assert_eq!(unb64(&b64(b"hello world")).unwrap(), b"hello world");
        let _ = std::fs::remove_file(&d);
    }
}
