//! 存档信息：名字、种子、模式、版本、出生点、玩家在哪（ROADMAP 5.5、5.16）。
//!
//! 各版本写法不同：
//! - 1.15 及更早：种子在 `Data.RandomSeed`，玩家在 `Data.Player`，维度是数字
//! - 1.16 起：种子在 `Data.WorldGenSettings.seed`，维度是 `minecraft:overworld` 这样的名字
//! - 26.x 起：种子挪到 `data/minecraft/world_gen_settings.dat`，玩家挪到 `players/data/<uuid>.dat`，
//!   难度写成 `Data.difficulty_settings`，出生点是 `Data.spawn.pos`

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use super::nbt::{self, Tag};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub folder: String,
    pub name: String,
    /// 种子写成字符串：JS 的数字放不下 64 位整数
    pub seed: Option<String>,
    /// survival / creative / adventure / spectator
    pub mode: String,
    pub hardcore: bool,
    /// peaceful / easy / normal / hard
    pub difficulty: Option<String>,
    pub cheats: bool,
    pub version: Option<String>,
    pub data_version: Option<i64>,
    pub last_played: i64,
    /// 游戏里过了多少天
    pub days: Option<i64>,
    pub spawn: Option<[i64; 3]>,
    pub player: Option<Player>,
    pub modded: bool,
    pub size: u64,
    /// 存档图标（data: URL），没有就是 None
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    /// overworld / the_nether / the_end，或者 Mod 的维度全名
    pub dimension: String,
    pub pos: [f64; 3],
    pub health: Option<f64>,
    pub level: Option<i64>,
}

const MODES: [&str; 4] = ["survival", "creative", "adventure", "spectator"];
const DIFFS: [&str; 4] = ["peaceful", "easy", "normal", "hard"];

pub fn info(dir: &Path) -> Result<WorldInfo> {
    let root = nbt::read_file(&dir.join("level.dat"))?;
    let data = root.get("Data").cloned().unwrap_or(Tag::Compound(vec![]));
    let int = |p: &str| data.path(p).and_then(Tag::as_i64);
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

    let seed = int("WorldGenSettings.seed").or_else(|| int("RandomSeed")).or_else(|| {
        nbt::read_file(&dir.join("data/minecraft/world_gen_settings.dat")).ok().and_then(|t| t.path("data.seed").and_then(Tag::as_i64))
    });
    let difficulty = data
        .path("difficulty_settings.difficulty")
        .and_then(Tag::as_str)
        .map(String::from)
        .or_else(|| int("Difficulty").and_then(|d| DIFFS.get(d as usize)).map(|s| s.to_string()));
    let hardcore = int("hardcore").or_else(|| int("difficulty_settings.hardcore")).unwrap_or(0) != 0;
    let spawn = match (int("SpawnX"), int("SpawnY"), int("SpawnZ")) {
        (Some(x), Some(y), Some(z)) => Some([x, y, z]),
        _ => data.path("spawn.pos").and_then(Tag::as_ints).filter(|v| v.len() == 3).map(|v| [v[0] as i64, v[1] as i64, v[2] as i64]),
    };

    Ok(WorldInfo {
        name: data.get("LevelName").and_then(Tag::as_str).filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| folder.clone()),
        seed: seed.map(|s| s.to_string()),
        mode: int("GameType").and_then(|g| MODES.get(g as usize)).unwrap_or(&"survival").to_string(),
        hardcore,
        difficulty,
        cheats: int("allowCommands").unwrap_or(0) != 0,
        version: data.path("Version.Name").and_then(Tag::as_str).map(String::from),
        data_version: int("DataVersion").or_else(|| int("Version.Id")),
        last_played: int("LastPlayed").unwrap_or(0),
        days: int("Time").map(|t| t / 24000),
        spawn,
        player: player(dir, &data),
        modded: int("WasModded").unwrap_or(0) != 0 || root.get("fml").is_some() || root.get("forge").is_some(),
        size: dir_size(dir),
        icon: icon(&dir.join("icon.png")),
        folder,
    })
}

/// 单人世界的玩家：老版本在 level.dat 里，新版本在 players/data/ 或 playerdata/ 下（取最近改过的）
fn player(dir: &Path, data: &Tag) -> Option<Player> {
    if let Some(p) = data.get("Player") {
        return parse_player(p);
    }
    if let Some(u) = data.get("singleplayer_uuid").and_then(Tag::as_ints).filter(|v| v.len() == 4) {
        let f = dir.join("players/data").join(format!("{}.dat", uuid_of(u)));
        if let Some(p) = nbt::read_file(&f).ok().and_then(|t| parse_player(&t)) {
            return Some(p);
        }
    }
    let newest = ["players/data", "playerdata"]
        .iter()
        .flat_map(|d| std::fs::read_dir(dir.join(d)).into_iter().flatten().flatten())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".dat"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
    nbt::read_file(&newest.path()).ok().and_then(|t| parse_player(&t))
}

fn parse_player(p: &Tag) -> Option<Player> {
    let pos = p.get("Pos")?.as_list()?;
    if pos.len() != 3 {
        return None;
    }
    let dimension = match p.get("Dimension") {
        Some(Tag::String(s)) => s.trim_start_matches("minecraft:").to_string(),
        Some(t) => match t.as_i64() {
            Some(-1) => "the_nether".into(),
            Some(1) => "the_end".into(),
            _ => "overworld".into(),
        },
        None => "overworld".into(),
    };
    Some(Player {
        dimension,
        pos: [pos[0].as_f64()?, pos[1].as_f64()?, pos[2].as_f64()?],
        health: p.get("Health").and_then(Tag::as_f64),
        level: p.get("XpLevel").and_then(Tag::as_i64),
    })
}

fn uuid_of(v: &[i32]) -> String {
    let h: String = v.iter().map(|x| format!("{:08x}", *x as u32)).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

fn icon(p: &Path) -> Option<String> {
    let b = std::fs::read(p).ok().filter(|b| b.len() < 256 * 1024 && b.starts_with(b"\x89PNG"))?;
    Some(format!("data:image/png;base64,{}", crate::base::util::base64(&b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(data: Vec<(&str, Tag)>) -> Tag {
        Tag::Compound(vec![("Data".into(), Tag::Compound(data.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))])
    }

    #[test]
    fn old_and_new_layouts() {
        let base = std::env::temp_dir().join(format!("ls-world-{}", std::process::id()));
        // 1.12：种子、玩家都在 level.dat
        let old = base.join("old");
        std::fs::create_dir_all(&old).unwrap();
        nbt::write_file(
            &old.join("level.dat"),
            &level(vec![
                ("LevelName", Tag::String("老世界".into())),
                ("RandomSeed", Tag::Long(-123456789012345)),
                ("GameType", Tag::Int(1)),
                ("Difficulty", Tag::Byte(3)),
                ("SpawnX", Tag::Int(10)),
                ("SpawnY", Tag::Int(64)),
                ("SpawnZ", Tag::Int(-5)),
                ("Time", Tag::Long(24000 * 7 + 5)),
                ("Player", Tag::Compound(vec![("Pos".into(), Tag::List(6, vec![Tag::Double(1.0), Tag::Double(70.0), Tag::Double(2.0)])), ("Dimension".into(), Tag::Int(-1))])),
            ]),
        )
        .unwrap();
        let w = info(&old).unwrap();
        assert_eq!((w.name.as_str(), w.seed.as_deref(), w.mode.as_str(), w.difficulty.as_deref()), ("老世界", Some("-123456789012345"), "creative", Some("hard")));
        assert_eq!(w.spawn, Some([10, 64, -5]));
        assert_eq!(w.days, Some(7));
        assert_eq!(w.player.as_ref().unwrap().dimension, "the_nether");

        // 26.x：种子和玩家都挪出去了
        let new = base.join("new");
        std::fs::create_dir_all(new.join("data/minecraft")).unwrap();
        std::fs::create_dir_all(new.join("players/data")).unwrap();
        let uuid = [0x544a04ad_u32 as i32, 0x6be642d0, 0x91f3bad5_u32 as i32, 0x2febf1e8];
        nbt::write_file(
            &new.join("level.dat"),
            &level(vec![
                ("LevelName", Tag::String("新的世界".into())),
                ("difficulty_settings", Tag::Compound(vec![("difficulty".into(), Tag::String("normal".into())), ("hardcore".into(), Tag::Byte(1))])),
                ("spawn", Tag::Compound(vec![("pos".into(), Tag::IntArray(vec![0, 80, 16]))])),
                ("singleplayer_uuid", Tag::IntArray(uuid.to_vec())),
                ("Version", Tag::Compound(vec![("Name".into(), Tag::String("26.2".into()))])),
            ]),
        )
        .unwrap();
        nbt::write_file(&new.join("data/minecraft/world_gen_settings.dat"), &Tag::Compound(vec![("data".into(), Tag::Compound(vec![("seed".into(), Tag::Long(4482507761930407068))]))])).unwrap();
        nbt::write_file(
            &new.join("players/data/544a04ad-6be6-42d0-91f3-bad52febf1e8.dat"),
            &Tag::Compound(vec![("Pos".into(), Tag::List(6, vec![Tag::Double(0.5), Tag::Double(80.0), Tag::Double(16.5)])), ("Dimension".into(), Tag::String("minecraft:overworld".into())), ("XpLevel".into(), Tag::Int(3))]),
        )
        .unwrap();
        let w = info(&new).unwrap();
        assert_eq!(w.seed.as_deref(), Some("4482507761930407068"));
        assert_eq!((w.difficulty.as_deref(), w.hardcore, w.version.as_deref()), (Some("normal"), true, Some("26.2")));
        assert_eq!(w.spawn, Some([0, 80, 16]));
        let p = w.player.unwrap();
        assert_eq!((p.dimension.as_str(), p.level), ("overworld", Some(3)));
        let _ = std::fs::remove_dir_all(&base);
    }
}
