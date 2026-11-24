//! NBT：Minecraft 存档用的二进制格式。level.dat 这类文件是 gzip 压缩的整棵树。
//! 字符串是 Java 的「改良 UTF-8」（`\0` 写成两个字节，BMP 以外的字拆成代理对各写三个字节）。
//! 写的那一半现在只有测试在用，留给世界工具（M13：改游戏规则、局外指令）。
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// 列表里的元素类型（空列表也要记着）
    List(u8, Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(v) => v.iter().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }

    /// 按点分路径取：`Data.WorldGenSettings.seed`
    pub fn path(&self, p: &str) -> Option<&Tag> {
        p.split('.').try_fold(self, |t, k| t.get(k))
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Tag> {
        match self {
            Tag::Compound(v) => v.iter_mut().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }

    /// 设一个子项（没有就加上）
    pub fn set(&mut self, key: &str, val: Tag) {
        if let Tag::Compound(v) = self {
            match v.iter_mut().find(|(k, _)| k == key) {
                Some((_, t)) => *t = val,
                None => v.push((key.to_string(), val)),
            }
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Tag::Byte(n) => Some(n as i64),
            Tag::Short(n) => Some(n as i64),
            Tag::Int(n) => Some(n as i64),
            Tag::Long(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Tag::Float(n) => Some(n as f64),
            Tag::Double(n) => Some(n),
            _ => self.as_i64().map(|n| n as f64),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Tag]> {
        match self {
            Tag::List(_, v) => Some(v),
            _ => None,
        }
    }

    pub fn as_ints(&self) -> Option<&[i32]> {
        match self {
            Tag::IntArray(v) => Some(v),
            _ => None,
        }
    }
}

// ---------- 读 ----------

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.b.len() - self.at < n {
            bail!("NBT 数据不完整");
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into()?))
    }
    fn len(&mut self) -> Result<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > self.b.len() - self.at {
            bail!("NBT 长度不对：{}", n);
        }
        Ok(n as usize)
    }
    fn string(&mut self) -> Result<String> {
        let n = u16::from_be_bytes(self.take(2)?.try_into()?) as usize;
        Ok(mutf8_decode(self.take(n)?))
    }
    fn tag(&mut self, id: u8, depth: u32) -> Result<Tag> {
        if depth > 512 {
            bail!("NBT 嵌套太深");
        }
        Ok(match id {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(self.i16()?),
            3 => Tag::Int(self.i32()?),
            4 => Tag::Long(self.i64()?),
            5 => Tag::Float(f32::from_bits(self.i32()? as u32)),
            6 => Tag::Double(f64::from_bits(self.i64()? as u64)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.iter().map(|&x| x as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let et = self.u8()?;
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.tag(et, depth + 1)?);
                }
                Tag::List(et, v)
            }
            10 => {
                let mut v = vec![];
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let k = self.string()?;
                    v.push((k, self.tag(t, depth + 1)?));
                }
                Tag::Compound(v)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(self.i32()?);
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(self.i64()?);
                }
                Tag::LongArray(v)
            }
            _ => bail!("不认识的 NBT 类型 {}", id),
        })
    }
}

/// 读一棵没压缩的 NBT，返回根的名字和内容
pub fn read(b: &[u8]) -> Result<(String, Tag)> {
    let mut r = Reader { b, at: 0 };
    let id = r.u8()?;
    if id != 10 {
        bail!("不是 NBT 文件");
    }
    let name = r.string()?;
    Ok((name, r.tag(10, 0)?))
}

/// 读文件，gzip 压缩的和没压缩的都行
pub fn read_file(p: &Path) -> Result<Tag> {
    let raw = std::fs::read(p).with_context(|| format!("读不了 {}", p.display()))?;
    let data = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut out = vec![];
        flate2::read::GzDecoder::new(&raw[..]).read_to_end(&mut out).with_context(|| format!("{} 解压失败", p.display()))?;
        out
    } else {
        raw
    };
    read(&data).map(|(_, t)| t).with_context(|| format!("{} 读不懂", p.display()))
}

// ---------- 写 ----------

fn put_str(out: &mut Vec<u8>, s: &str) {
    let b = mutf8_encode(s);
    let n = b.len().min(u16::MAX as usize);
    out.extend_from_slice(&(n as u16).to_be_bytes());
    out.extend_from_slice(&b[..n]);
}

fn put(out: &mut Vec<u8>, t: &Tag) {
    match t {
        Tag::Byte(n) => out.push(*n as u8),
        Tag::Short(n) => out.extend_from_slice(&n.to_be_bytes()),
        Tag::Int(n) => out.extend_from_slice(&n.to_be_bytes()),
        Tag::Long(n) => out.extend_from_slice(&n.to_be_bytes()),
        Tag::Float(n) => out.extend_from_slice(&n.to_bits().to_be_bytes()),
        Tag::Double(n) => out.extend_from_slice(&n.to_bits().to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend(v.iter().map(|&x| x as u8));
        }
        Tag::String(s) => put_str(out, s),
        Tag::List(et, v) => {
            out.push(if v.is_empty() { *et } else { v[0].id() });
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                put(out, x);
            }
        }
        Tag::Compound(v) => {
            for (k, x) in v {
                out.push(x.id());
                put_str(out, k);
                put(out, x);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

pub fn write(name: &str, root: &Tag) -> Vec<u8> {
    let mut out = vec![10];
    put_str(&mut out, name);
    put(&mut out, root);
    out
}

/// 写成 gzip 文件。和游戏一样先留一份 `<文件名>_old`，再写临时文件换上去，写到一半断电也不会坏
pub fn write_file(p: &Path, root: &Tag) -> Result<()> {
    let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::default());
    gz.write_all(&write("", root))?;
    let data = gz.finish()?;
    if p.is_file() {
        let old = p.with_file_name(format!("{}_old", p.file_name().unwrap_or_default().to_string_lossy()));
        std::fs::copy(p, &old).with_context(|| format!("备份 {} 失败", p.display()))?;
    }
    let tmp = p.with_extension("ls-tmp");
    std::fs::write(&tmp, data)?;
    crate::base::util::move_file(&tmp, p).with_context(|| format!("换不上 {}（游戏是不是开着这个世界？）", p.display()))?;
    Ok(())
}

// ---------- 改良 UTF-8 ----------

fn mutf8_decode(b: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c & 0x80 == 0 {
            units.push(c as u16);
            i += 1;
        } else if c & 0xE0 == 0xC0 && i + 1 < b.len() {
            units.push((((c & 0x1F) as u16) << 6) | (b[i + 1] & 0x3F) as u16);
            i += 2;
        } else if c & 0xF0 == 0xE0 && i + 2 < b.len() {
            units.push((((c & 0x0F) as u16) << 12) | (((b[i + 1] & 0x3F) as u16) << 6) | (b[i + 2] & 0x3F) as u16);
            i += 3;
        } else {
            // 真正的 4 字节 UTF-8（别的工具写的），按普通 UTF-8 收下
            let end = (i + 4).min(b.len());
            for ch in String::from_utf8_lossy(&b[i..end]).encode_utf16() {
                units.push(ch);
            }
            i = end;
        }
    }
    String::from_utf16_lossy(&units)
}

fn mutf8_encode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for u in s.encode_utf16() {
        match u {
            1..=0x7F => out.push(u as u8),
            0 | 0x80..=0x7FF => {
                out.push(0xC0 | (u >> 6) as u8);
                out.push(0x80 | (u & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (u >> 12) as u8);
                out.push(0x80 | ((u >> 6) & 0x3F) as u8);
                out.push(0x80 | (u & 0x3F) as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let t = Tag::Compound(vec![
            ("Data".into(), Tag::Compound(vec![
                ("LevelName".into(), Tag::String("新的世界 😀\0".into())),
                ("RandomSeed".into(), Tag::Long(-4482507761930407068)),
                ("GameType".into(), Tag::Int(1)),
                ("Pos".into(), Tag::List(6, vec![Tag::Double(1.5), Tag::Double(64.0), Tag::Double(-3.25)])),
                ("Empty".into(), Tag::List(10, vec![])),
                ("uuid".into(), Tag::IntArray(vec![1, -2, 3, 4])),
                ("hardcore".into(), Tag::Byte(0)),
            ])),
        ]);
        let (name, back) = read(&write("", &t)).unwrap();
        assert_eq!(name, "");
        assert_eq!(back, t);
        assert_eq!(back.path("Data.LevelName").and_then(Tag::as_str), Some("新的世界 😀\0"));
        assert_eq!(back.path("Data.RandomSeed").and_then(Tag::as_i64), Some(-4482507761930407068));
        // 改良 UTF-8：\0 是两个字节，表情是两个三字节
        assert_eq!(mutf8_encode("\0"), vec![0xC0, 0x80]);
        assert_eq!(mutf8_encode("😀").len(), 6);
    }

    #[test]
    fn rejects_garbage() {
        assert!(read(&[10, 0, 0, 8]).is_err());
        assert!(read(&[1, 2, 3]).is_err());
        assert!(read(&[10, 0, 0, 9, 0, 1, b'a', 1, 0x7f, 0xff, 0xff, 0xff]).is_err());
    }
}
