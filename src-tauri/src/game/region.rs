//! 读存档的 region（.mca）文件（M13，ROADMAP 5.16）：画俯视地图、列出已经生成的建筑。
//! 认 1.13 起的区块格式（1.18+ 的 `sections` / `block_states`，1.13–1.17 的 `Level.Sections`）；1.12 及以前用数字方块 id，地图先不画。
//! 目录：老写法 `region`、`DIM-1/region`、`DIM1/region`；26.x 起 `dimensions/minecraft/<维度>/region`。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use super::nbt::{self, Tag};

/// 维度的 region 目录
pub fn region_dir(world: &Path, dim: &str) -> PathBuf {
    let (old, new) = match dim {
        "nether" => ("DIM-1/region", "dimensions/minecraft/the_nether/region"),
        "end" => ("DIM1/region", "dimensions/minecraft/the_end/region"),
        _ => ("region", "dimensions/minecraft/overworld/region"),
    };
    let n = world.join(new);
    if n.is_dir() {
        n
    } else {
        world.join(old)
    }
}

/// 有哪些 region：(rx, rz)
pub fn list(world: &Path, dim: &str) -> Vec<(i32, i32)> {
    let mut out = vec![];
    for e in std::fs::read_dir(region_dir(world, dim)).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let p: Vec<&str> = n.split('.').collect();
        if p.len() == 4 && p[0] == "r" && p[3] == "mca" && e.metadata().map(|m| m.len() > 8192).unwrap_or(false) {
            if let (Ok(x), Ok(z)) = (p[1].parse(), p[2].parse()) {
                out.push((x, z));
            }
        }
    }
    out.sort();
    out
}

/// 一个 region 文件里的所有区块（解压后的 NBT）
pub fn chunks(path: &Path) -> Result<Vec<Tag>> {
    let b = std::fs::read(path).with_context(|| format!("读不了 {}", path.display()))?;
    if b.len() < 8192 {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for i in 0..1024 {
        let e = &b[i * 4..i * 4 + 4];
        let off = ((e[0] as usize) << 16 | (e[1] as usize) << 8 | e[2] as usize) * 4096;
        if off == 0 || off + 5 > b.len() {
            continue;
        }
        let len = u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]) as usize;
        let kind = b[off + 4];
        if len < 1 || off + 4 + len > b.len() {
            continue;
        }
        let data = &b[off + 5..off + 4 + len];
        let raw = match kind & 0x7f {
            1 => {
                let mut v = vec![];
                flate2::read::GzDecoder::new(data).read_to_end(&mut v).ok().map(|_| v)
            }
            2 => {
                let mut v = vec![];
                flate2::read::ZlibDecoder::new(data).read_to_end(&mut v).ok().map(|_| v)
            }
            3 => Some(data.to_vec()),
            // 4 是 LZ4（1.20.5 起可选），外置的 .mcc 也先跳过
            _ => None,
        };
        if let Some(raw) = raw {
            if let Ok((_, t)) = nbt::read(&raw) {
                out.push(t);
            }
        }
    }
    Ok(out)
}

/// 区块的「本体」：1.18+ 就是根，老格式在 Level 下面
fn body(c: &Tag) -> &Tag {
    c.get("Level").unwrap_or(c)
}

fn data_version(c: &Tag) -> i64 {
    c.get("DataVersion").and_then(Tag::as_i64).unwrap_or(0)
}

/// 压在 long 数组里的定长整数：1.16（DataVersion 2529）起不跨 long，之前跨
fn unpack(longs: &[i64], bits: usize, i: usize, span: bool) -> usize {
    let mask = (1u64 << bits) - 1;
    if span {
        let bit = i * bits;
        let (li, off) = (bit / 64, bit % 64);
        let mut v = (longs.get(li).copied().unwrap_or(0) as u64) >> off;
        if off + bits > 64 {
            v |= (longs.get(li + 1).copied().unwrap_or(0) as u64) << (64 - off);
        }
        (v & mask) as usize
    } else {
        let per = 64 / bits;
        let (li, off) = (i / per, (i % per) * bits);
        ((longs.get(li).copied().unwrap_or(0) as u64 >> off) & mask) as usize
    }
}

struct Section<'a> {
    palette: Vec<&'a str>,
    data: Option<&'a [i64]>,
    bits: usize,
}

fn long_array(t: Option<&Tag>) -> Option<&[i64]> {
    match t {
        Some(Tag::LongArray(v)) => Some(v),
        _ => None,
    }
}

/// 一个区块里各高度段的方块
struct ChunkBlocks<'a> {
    sections: HashMap<i32, Section<'a>>,
    span: bool,
}

impl<'a> ChunkBlocks<'a> {
    fn new(c: &'a Tag) -> ChunkBlocks<'a> {
        let span = data_version(c) < 2529;
        let b = body(c);
        let list = b.get("sections").or_else(|| b.get("Sections")).and_then(Tag::as_list).unwrap_or(&[]);
        let mut sections = HashMap::new();
        for s in list {
            let y = s.get("Y").and_then(Tag::as_i64).unwrap_or(0) as i32;
            let (pal, data) = match s.get("block_states") {
                Some(bs) => (bs.get("palette").and_then(Tag::as_list), long_array(bs.get("data"))),
                None => (s.get("Palette").and_then(Tag::as_list), long_array(s.get("BlockStates"))),
            };
            let Some(pal) = pal else { continue };
            let palette: Vec<&str> = pal.iter().map(|p| p.get("Name").and_then(Tag::as_str).unwrap_or("minecraft:air")).collect();
            let n = palette.len().max(1);
            let bits = (usize::BITS - (n - 1).leading_zeros()).max(4) as usize;
            sections.insert(y, Section { palette, data, bits });
        }
        ChunkBlocks { sections, span }
    }

    fn block(&self, x: usize, y: i32, z: usize) -> &str {
        let Some(s) = self.sections.get(&y.div_euclid(16)) else { return "minecraft:air" };
        if s.palette.len() == 1 {
            return s.palette[0];
        }
        let Some(d) = s.data else { return s.palette[0] };
        let i = (y.rem_euclid(16) as usize) * 256 + z * 16 + x;
        s.palette.get(unpack(d, s.bits, i, self.span)).copied().unwrap_or("minecraft:air")
    }

    fn min_y(&self) -> i32 {
        self.sections.keys().min().copied().unwrap_or(0) * 16
    }
}

// ---------- 颜色 ----------

fn rgb(h: u32) -> [u8; 3] {
    [(h >> 16) as u8, (h >> 8) as u8, h as u8]
}

/// 方块 → 地图颜色（参照原版地图的配色；Mod 方块按名字猜）
pub fn color(name: &str) -> Option<[u8; 3]> {
    let n = name.rsplit(':').next().unwrap_or(name);
    if n.ends_with("air") || n == "light" || n == "barrier" || n == "structure_void" {
        return None;
    }
    let exact: &[(&str, u32)] = &[
        ("grass_block", 0x7FB238), ("water", 0x4040FF), ("bubble_column", 0x4040FF), ("lava", 0xFF5A00), ("sand", 0xF7E9A3), ("red_sand", 0xD87F33),
        ("snow", 0xFFFFFF), ("snow_block", 0xFFFFFF), ("powder_snow", 0xFFFFFF), ("ice", 0xA0A0FF), ("packed_ice", 0xA0A0FF), ("blue_ice", 0xA0A0FF),
        ("gravel", 0x837E7E), ("clay", 0xA4A8B8), ("dirt", 0x976D4D), ("coarse_dirt", 0x976D4D), ("rooted_dirt", 0x976D4D), ("podzol", 0x815631), ("mycelium", 0x7F3FB2),
        ("dirt_path", 0x976D4D), ("farmland", 0x976D4D), ("mud", 0x575C5C), ("netherrack", 0x700200), ("soul_sand", 0x664C33), ("soul_soil", 0x664C33),
        ("end_stone", 0xF7E9A3), ("obsidian", 0x191919), ("bedrock", 0x707070), ("crimson_nylium", 0xBD3031), ("warped_nylium", 0x167E86), ("basalt", 0x191919),
        ("blackstone", 0x191919), ("magma_block", 0x700200), ("glowstone", 0xF7E9A3), ("moss_block", 0x667F33), ("sugar_cane", 0x007C00), ("cactus", 0x007C00),
        ("pumpkin", 0xD87F33), ("melon", 0x7FCC19), ("hay_block", 0xE5E533), ("bamboo", 0x667F33), ("kelp", 0x4040FF), ("seagrass", 0x4040FF), ("tall_seagrass", 0x4040FF),
    ];
    if let Some((_, c)) = exact.iter().find(|(k, _)| *k == n) {
        return Some(rgb(*c));
    }
    let guess: &[(&str, u32)] = &[
        ("leaves", 0x2F7C14), ("grass", 0x6FA02F), ("fern", 0x5C8A2A), ("flower", 0x7FB238), ("tulip", 0x7FB238), ("vine", 0x2F7C14), ("azalea", 0x2F7C14),
        ("log", 0x8F7748), ("wood", 0x8F7748), ("planks", 0x8F7748), ("stem", 0x8F7748), ("fence", 0x8F7748), ("slab", 0x8F8F8F), ("stairs", 0x8F8F8F),
        ("deepslate", 0x646464), ("stone", 0x707070), ("andesite", 0x707070), ("diorite", 0xFFFCF5), ("granite", 0x976D4D), ("tuff", 0x575C5C), ("calcite", 0xD1B1A1),
        ("ore", 0x707070), ("cobble", 0x707070), ("brick", 0x993333), ("terracotta", 0x9F5224), ("concrete", 0x8F8F8F), ("wool", 0xC7C7C7), ("glass", 0xC0D6E0),
        ("sandstone", 0xF7E9A3), ("prismarine", 0x4C7F99), ("coral", 0xF0617A), ("mushroom", 0x7F3FB2), ("snow", 0xFFFFFF), ("ice", 0xA0A0FF), ("water", 0x4040FF),
        ("crop", 0x7FB238), ("wheat", 0xE5E533), ("carrots", 0x7FB238), ("potatoes", 0x7FB238), ("dirt", 0x976D4D), ("nether", 0x700200), ("quartz", 0xFFFCF5),
    ];
    for (k, c) in guess {
        if n.contains(k) {
            return Some(rgb(*c));
        }
    }
    Some(rgb(0x8A8A8A))
}

fn is_water(n: &str) -> bool {
    let n = n.rsplit(':').next().unwrap_or(n);
    matches!(n, "water" | "bubble_column" | "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass")
}

/// 画一个 region：512×512 的 RGBA（没生成的地方透明）。nether 从 y=120 往下找第一块露在空气下面的方块
pub fn render(path: &Path, dim: &str) -> Result<Vec<u8>> {
    let cs = chunks(path)?;
    let mut img = vec![0u8; 512 * 512 * 4];
    let mut heights = vec![i32::MIN; 512 * 512];
    for c in &cs {
        let b = body(c);
        let (Some(cx), Some(cz)) = (b.get("xPos").and_then(Tag::as_i64), b.get("zPos").and_then(Tag::as_i64)) else { continue };
        let status = b.get("Status").and_then(Tag::as_str).unwrap_or("full");
        if !status.ends_with("full") && !status.ends_with("spawn") && !status.ends_with("heightmaps") && !status.ends_with("postprocessed") {
            continue;
        }
        let blocks = ChunkBlocks::new(c);
        if blocks.sections.is_empty() {
            continue;
        }
        let min_y = b.get("yPos").and_then(Tag::as_i64).map(|y| y as i32 * 16).unwrap_or_else(|| blocks.min_y().min(0));
        let hm = b.get("Heightmaps").and_then(|h| long_array(h.get("MOTION_BLOCKING")).or_else(|| long_array(h.get("WORLD_SURFACE"))));
        let span = data_version(c) < 2529;
        let top = blocks.sections.keys().max().copied().unwrap_or(0) * 16 + 15;
        let (ox, oz) = ((cx.rem_euclid(32) * 16) as usize, (cz.rem_euclid(32) * 16) as usize);
        for z in 0..16 {
            for x in 0..16 {
                let mut y = match (dim, hm) {
                    ("nether", _) | (_, None) => {
                        // 从上往下：先找空气，再找下面第一块实心
                        let start = if dim == "nether" { 120.min(top) } else { top };
                        let mut y = start;
                        let mut seen_air = dim != "nether";
                        loop {
                            if y < min_y {
                                break i32::MIN;
                            }
                            let solid = color(blocks.block(x, y, z)).is_some();
                            if !solid {
                                seen_air = true;
                            } else if seen_air {
                                break y;
                            }
                            y -= 1;
                        }
                    }
                    (_, Some(h)) => min_y + unpack(h, 9, z * 16 + x, span) as i32 - 1,
                };
                if y == i32::MIN || y < min_y {
                    continue;
                }
                // 高度图上面可能是透明的东西（花、草），往下找第一块有颜色的
                let mut name = blocks.block(x, y, z);
                let mut guard = 0;
                while color(name).is_none() && y > min_y && guard < 8 {
                    y -= 1;
                    name = blocks.block(x, y, z);
                    guard += 1;
                }
                let Some(mut col) = color(name) else { continue };
                // 水：看深度调深浅
                if is_water(name) {
                    let mut d = 0;
                    while d < 16 && is_water(blocks.block(x, y - d - 1, z)) {
                        d += 1;
                    }
                    let k = 1.0 - (d as f32 / 24.0);
                    col = [(col[0] as f32 * k) as u8, (col[1] as f32 * k) as u8, (col[2] as f32 * k) as u8];
                }
                let (px, pz) = (ox + x, oz + z);
                heights[pz * 512 + px] = y;
                let i = (pz * 512 + px) * 4;
                img[i..i + 3].copy_from_slice(&col);
                img[i + 3] = 255;
            }
        }
    }
    // 光影：比北边那格高就亮一点，低就暗一点（和原版地图一样的效果）
    for z in 1..512 {
        for x in 0..512 {
            let (h, n) = (heights[z * 512 + x], heights[(z - 1) * 512 + x]);
            if h == i32::MIN || n == i32::MIN {
                continue;
            }
            let k = if h > n { 1.12 } else if h < n { 0.86 } else { 1.0 };
            let i = (z * 512 + x) * 4;
            for c in 0..3 {
                img[i + c] = (img[i + c] as f32 * k).min(255.0) as u8;
            }
        }
    }
    Ok(img)
}

pub fn png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let mut out = vec![];
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header()?;
        wr.write_image_data(rgba)?;
    }
    Ok(out)
}

// ---------- 建筑 ----------

#[derive(Debug, Clone, Serialize)]
pub struct Structure {
    /// minecraft:village_plains 这种
    pub id: String,
    pub x: i32,
    pub z: i32,
}

/// 已经生成的区块里记着的建筑起点
pub fn structures(world: &Path, dim: &str) -> Result<Vec<Structure>> {
    let mut out: Vec<Structure> = vec![];
    let regions = list(world, dim);
    if regions.is_empty() {
        bail!("这个维度还没有生成过区块");
    }
    for (rx, rz) in regions {
        let p = region_dir(world, dim).join(format!("r.{rx}.{rz}.mca"));
        for c in chunks(&p).unwrap_or_default() {
            let b = body(&c);
            let starts = b.get("structures").and_then(|s| s.get("starts")).or_else(|| b.get("Structures").and_then(|s| s.get("Starts")));
            let Some(Tag::Compound(starts)) = starts else { continue };
            for (name, st) in starts {
                let id = st.get("id").and_then(Tag::as_str).unwrap_or(name);
                if id == "INVALID" {
                    continue;
                }
                // 用第一个部件的包围盒中心；没有就用区块中心
                let bb = st.get("Children").and_then(Tag::as_list).and_then(|l| l.first()).and_then(|ch| ch.get("BB")).and_then(Tag::as_ints);
                let (x, z) = match bb {
                    Some(v) if v.len() == 6 => ((v[0] + v[3]) / 2, (v[2] + v[5]) / 2),
                    _ => {
                        let cx = st.get("ChunkX").and_then(Tag::as_i64).unwrap_or(0) as i32;
                        let cz = st.get("ChunkZ").and_then(Tag::as_i64).unwrap_or(0) as i32;
                        (cx * 16 + 8, cz * 16 + 8)
                    }
                };
                let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{}", id.to_lowercase()) };
                if !out.iter().any(|s| s.id == id && (s.x - x).abs() < 64 && (s.z - z).abs() < 64) {
                    out.push(Structure { id, x, z });
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacking() {
        // 不跨 long：每个 long 放 16 个 4 位的数
        let v: i64 = 0x0000_0000_0000_4321;
        assert_eq!(unpack(&[v], 4, 0, false), 1);
        assert_eq!(unpack(&[v], 4, 3, false), 4);
        // 跨 long：5 位一个，第 12 个从第 60 位开始，横跨两个 long
        let a = (0b10101i64 << 60) as i64;
        let b = 0b1i64;
        assert_eq!(unpack(&[a, b], 5, 12, true), 0b10101 & 0b01111 | (1 << 4));
        assert!(color("minecraft:air").is_none() && color("minecraft:cave_air").is_none());
        assert_eq!(color("minecraft:grass_block"), Some([0x7F, 0xB2, 0x38]));
        assert_eq!(color("create:andesite_casing"), Some(rgb(0x707070)));
    }
}
