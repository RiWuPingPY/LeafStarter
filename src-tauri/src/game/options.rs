//! 设置模板（ROADMAP 5.5）：把一个实例 options.txt 里的键位、视角、画面、语言和声音套到另一个实例上。
//!
//! 1.12 及更早的键位是 LWJGL2 的数字码（`key_key.forward:17`，鼠标是 -100 起），
//! 1.13 起是名字（`key_key.forward:key.keyboard.w`）。两边格式不一样时逐个换算。
//! 目标实例还没有 options.txt 时，写成老格式、不写 `version`：游戏第一次读的时候会自己按数据修复器升级。

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Serialize;

/// 数字码 ↔ 名字。没列的键（媒体键之类）两边都不认就跳过
const KEYS: &[(i32, &str)] = &[
    (0, "key.keyboard.unknown"),
    (1, "key.keyboard.escape"),
    (2, "key.keyboard.1"),
    (3, "key.keyboard.2"),
    (4, "key.keyboard.3"),
    (5, "key.keyboard.4"),
    (6, "key.keyboard.5"),
    (7, "key.keyboard.6"),
    (8, "key.keyboard.7"),
    (9, "key.keyboard.8"),
    (10, "key.keyboard.9"),
    (11, "key.keyboard.0"),
    (12, "key.keyboard.minus"),
    (13, "key.keyboard.equal"),
    (14, "key.keyboard.backspace"),
    (15, "key.keyboard.tab"),
    (16, "key.keyboard.q"),
    (17, "key.keyboard.w"),
    (18, "key.keyboard.e"),
    (19, "key.keyboard.r"),
    (20, "key.keyboard.t"),
    (21, "key.keyboard.y"),
    (22, "key.keyboard.u"),
    (23, "key.keyboard.i"),
    (24, "key.keyboard.o"),
    (25, "key.keyboard.p"),
    (26, "key.keyboard.left.bracket"),
    (27, "key.keyboard.right.bracket"),
    (28, "key.keyboard.enter"),
    (29, "key.keyboard.left.control"),
    (30, "key.keyboard.a"),
    (31, "key.keyboard.s"),
    (32, "key.keyboard.d"),
    (33, "key.keyboard.f"),
    (34, "key.keyboard.g"),
    (35, "key.keyboard.h"),
    (36, "key.keyboard.j"),
    (37, "key.keyboard.k"),
    (38, "key.keyboard.l"),
    (39, "key.keyboard.semicolon"),
    (40, "key.keyboard.apostrophe"),
    (41, "key.keyboard.grave.accent"),
    (42, "key.keyboard.left.shift"),
    (43, "key.keyboard.backslash"),
    (44, "key.keyboard.z"),
    (45, "key.keyboard.x"),
    (46, "key.keyboard.c"),
    (47, "key.keyboard.v"),
    (48, "key.keyboard.b"),
    (49, "key.keyboard.n"),
    (50, "key.keyboard.m"),
    (51, "key.keyboard.comma"),
    (52, "key.keyboard.period"),
    (53, "key.keyboard.slash"),
    (54, "key.keyboard.right.shift"),
    (55, "key.keyboard.keypad.multiply"),
    (56, "key.keyboard.left.alt"),
    (57, "key.keyboard.space"),
    (58, "key.keyboard.caps.lock"),
    (59, "key.keyboard.f1"),
    (60, "key.keyboard.f2"),
    (61, "key.keyboard.f3"),
    (62, "key.keyboard.f4"),
    (63, "key.keyboard.f5"),
    (64, "key.keyboard.f6"),
    (65, "key.keyboard.f7"),
    (66, "key.keyboard.f8"),
    (67, "key.keyboard.f9"),
    (68, "key.keyboard.f10"),
    (69, "key.keyboard.num.lock"),
    (70, "key.keyboard.scroll.lock"),
    (71, "key.keyboard.keypad.7"),
    (72, "key.keyboard.keypad.8"),
    (73, "key.keyboard.keypad.9"),
    (74, "key.keyboard.keypad.subtract"),
    (75, "key.keyboard.keypad.4"),
    (76, "key.keyboard.keypad.5"),
    (77, "key.keyboard.keypad.6"),
    (78, "key.keyboard.keypad.add"),
    (79, "key.keyboard.keypad.1"),
    (80, "key.keyboard.keypad.2"),
    (81, "key.keyboard.keypad.3"),
    (82, "key.keyboard.keypad.0"),
    (83, "key.keyboard.keypad.decimal"),
    (87, "key.keyboard.f11"),
    (88, "key.keyboard.f12"),
    (100, "key.keyboard.f13"),
    (101, "key.keyboard.f14"),
    (102, "key.keyboard.f15"),
    (156, "key.keyboard.keypad.enter"),
    (157, "key.keyboard.right.control"),
    (179, "key.keyboard.keypad.comma"),
    (181, "key.keyboard.keypad.divide"),
    (183, "key.keyboard.print.screen"),
    (184, "key.keyboard.right.alt"),
    (197, "key.keyboard.pause"),
    (199, "key.keyboard.home"),
    (200, "key.keyboard.up"),
    (201, "key.keyboard.page.up"),
    (203, "key.keyboard.left"),
    (205, "key.keyboard.right"),
    (207, "key.keyboard.end"),
    (208, "key.keyboard.down"),
    (209, "key.keyboard.page.down"),
    (210, "key.keyboard.insert"),
    (211, "key.keyboard.delete"),
    (219, "key.keyboard.left.win"),
    (220, "key.keyboard.right.win"),
    (221, "key.keyboard.menu"),
    (-100, "key.mouse.left"),
    (-99, "key.mouse.right"),
    (-98, "key.mouse.middle"),
    (-97, "key.mouse.4"),
    (-96, "key.mouse.5"),
    (-95, "key.mouse.6"),
    (-94, "key.mouse.7"),
    (-93, "key.mouse.8"),
];

/// 改过名的键位（老名字, 新名字）
const RENAMED: &[(&str, &str)] = &[("key_key.swapHands", "key_key.swapOffhand")];

const VIEW: &[&str] = &[
    "fov", "fovEffectScale", "mouseSensitivity", "invertYMouse", "autoJump", "toggleCrouch", "toggleSprint", "bobView", "rawMouseInput",
    "mouseWheelSensitivity", "discrete_mouse_scroll", "attackIndicator", "mainHand", "sneakToggle", "sprintToggle",
];
const VIDEO: &[&str] = &[
    "renderDistance", "simulationDistance", "guiScale", "maxFps", "enableVsync", "gamma", "graphicsMode", "fancyGraphics", "ao", "renderClouds",
    "particles", "entityShadows", "entityDistanceScaling", "biomeBlendRadius", "fullscreen", "mipmapLevels", "useVbo", "prioritizeChunkUpdates",
    "menuBackgroundBlurriness", "darknessEffectScale", "glintSpeed", "glintStrength", "damageTiltStrength", "screenEffectScale", "chatScale",
    "chatOpacity", "textBackgroundOpacity", "backgroundForChatOnly",
];
const SOUND: &[&str] = &["lang", "forceUnicodeFont", "soundDevice", "directionalAudio", "showSubtitles", "narrator"];

pub const GROUPS: &[&str] = &["keys", "view", "video", "sound"];

fn group_of(key: &str) -> Option<&'static str> {
    if key.starts_with("key_") {
        Some("keys")
    } else if VIEW.contains(&key) {
        Some("view")
    } else if VIDEO.contains(&key) {
        Some("video")
    } else if SOUND.contains(&key) || key.starts_with("soundCategory_") {
        Some("sound")
    } else {
        None
    }
}

/// 读 options.txt，保留顺序；读不了就是空的
pub fn read(p: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

/// 键位用的是哪种写法；看不出来（没有键位行）就按版本号猜
fn modern(lines: &[(String, String)], mc: Option<&str>) -> bool {
    if let Some((_, v)) = lines.iter().find(|(k, _)| k.starts_with("key_")) {
        return v.starts_with("key.");
    }
    mc.map(is_modern_mc).unwrap_or(true)
}

/// 1.13 起（含 26.x 这样的新版本号和 18w 之后的快照）键位用名字
pub fn is_modern_mc(mc: &str) -> bool {
    let nums: Vec<u32> = mc.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
    if mc.contains('w') && !mc.contains('.') {
        // 快照：18w01a 之类
        return nums.first().is_some_and(|y| *y >= 18);
    }
    match nums.as_slice() {
        [1, minor, ..] => *minor >= 13,
        [major, ..] => *major > 1,
        _ => true,
    }
}

fn to_name(v: &str) -> Option<&'static str> {
    let n: i32 = v.trim().parse().ok()?;
    KEYS.iter().find(|(c, _)| *c == n).map(|(_, s)| *s)
}

fn to_code(v: &str) -> Option<i32> {
    KEYS.iter().find(|(_, s)| *s == v.trim()).map(|(c, _)| *c)
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub count: usize,
    /// 换算不了、跳过的键位
    pub skipped: Vec<String>,
    pub backup: Option<String>,
}

/// 把 `src` 里属于 `groups` 的设置套到 `dst`（options.txt）。dst 没有的项加在后面
pub fn apply(src: &Path, dst: &Path, dst_mc: Option<&str>, groups: &[String]) -> Result<Applied> {
    let from = read(src);
    if from.is_empty() {
        bail!("那个实例还没有设置文件（options.txt），先启动一次进到主菜单");
    }
    let fresh = !dst.is_file();
    let mut to = read(dst);
    let src_modern = modern(&from, None);
    // 目标还没有设置文件：写老格式，让游戏自己升级
    let dst_modern = if fresh { false } else { modern(&to, dst_mc) };
    let old_lang = dst_mc.is_some_and(|m| matches!(m.split('.').nth(1).and_then(|n| n.parse::<u32>().ok()), Some(n) if n < 11) && m.starts_with("1."));
    let mut out = Applied::default();

    for (k, v) in &from {
        let Some(g) = group_of(k) else { continue };
        if !groups.iter().any(|x| x == g) {
            continue;
        }
        let mut key = k.clone();
        let mut val = v.clone();
        if g == "keys" {
            for (old, new) in RENAMED {
                if dst_modern && key == *old {
                    key = new.to_string();
                } else if !dst_modern && key == *new {
                    key = old.to_string();
                }
            }
            if src_modern != dst_modern {
                let conv = if dst_modern { to_name(v).map(String::from) } else { to_code(v).map(|c| c.to_string()) };
                match conv {
                    Some(c) => val = c,
                    None => {
                        out.skipped.push(k.trim_start_matches("key_").to_string());
                        continue;
                    }
                }
            }
        }
        if key == "lang" {
            val = if old_lang { lang_old(&val) } else { val.to_lowercase() };
        }
        match to.iter_mut().find(|(x, _)| *x == key) {
            Some(e) => e.1 = val,
            None => to.push((key, val)),
        }
        out.count += 1;
    }
    if out.count == 0 {
        bail!("选的这几类在那个实例里都没有设置");
    }
    if !fresh {
        let bak = dst.with_file_name("options.txt.ls-bak");
        std::fs::copy(dst, &bak).context("备份原来的 options.txt 失败")?;
        out.backup = Some(bak.to_string_lossy().into_owned());
    } else if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    let text: String = to.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    std::fs::write(dst, text).with_context(|| format!("写不了 {}", dst.display()))?;
    Ok(out)
}

/// zh_cn → zh_CN（1.10 及更早的写法）
fn lang_old(v: &str) -> String {
    match v.split_once('_') {
        Some((a, b)) => format!("{}_{}", a.to_lowercase(), b.to_uppercase()),
        None => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        for (v, m) in [("1.12.2", false), ("1.8.9", false), ("1.13", true), ("1.20.1", true), ("26.2", true), ("26.2-snapshot-7", true), ("17w43a", false), ("24w14a", true)] {
            assert_eq!(is_modern_mc(v), m, "{v}");
        }
    }

    #[test]
    fn legacy_to_modern_and_back() {
        let d = std::env::temp_dir().join(format!("ls-opt-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (a, b, c) = (d.join("a.txt"), d.join("b.txt"), d.join("c.txt"));
        // 1.12.2：E 背包换成 R、跳跃换成鼠标中键、副手是 swapHands
        std::fs::write(&a, "version:1343\nfov:0.5\nlang:zh_cn\nkey_key.forward:17\nkey_key.inventory:19\nkey_key.jump:-98\nkey_key.swapHands:33\nkey_key.weird:999\nrenderDistance:8\n").unwrap();
        std::fs::write(&b, "version:3465\nfov:0.0\nkey_key.forward:key.keyboard.w\nkey_key.inventory:key.keyboard.e\nrenderDistance:12\n").unwrap();
        let r = apply(&a, &b, Some("1.20.1"), &["keys".into(), "view".into()]).unwrap();
        let got = read(&b);
        let get = |k: &str| got.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
        assert_eq!(get("key_key.inventory").as_deref(), Some("key.keyboard.r"));
        assert_eq!(get("key_key.jump").as_deref(), Some("key.mouse.middle"));
        assert_eq!(get("key_key.swapOffhand").as_deref(), Some("key.keyboard.f"));
        assert_eq!(get("fov").as_deref(), Some("0.5"));
        assert_eq!(get("renderDistance").as_deref(), Some("12"), "没选画面就不动");
        assert_eq!(get("version").as_deref(), Some("3465"), "版本号留着");
        assert_eq!(r.skipped, vec!["key.weird".to_string()]);
        assert!(r.backup.is_some());

        // 反过来：新版套到 1.8.9，语言写成老格式
        std::fs::write(&c, "fov:0.0\nkey_key.inventory:19\nlang:en_US\n").unwrap();
        apply(&b, &c, Some("1.8.9"), &["keys".into()]).unwrap();
        let got = read(&c);
        assert!(got.contains(&("key_key.inventory".into(), "19".into())));
        assert!(got.contains(&("key_key.swapHands".into(), "33".into())));
        std::fs::write(&a, "lang:zh_cn\n").unwrap();
        apply(&a, &c, Some("1.8.9"), &["sound".into()]).unwrap();
        assert!(read(&c).contains(&("lang".into(), "zh_CN".into())));

        // 目标还没有设置文件：老格式、不写版本号
        let fresh = d.join("new/options.txt");
        apply(&b, &fresh, Some("26.2"), &["keys".into()]).unwrap();
        let got = read(&fresh);
        assert!(got.contains(&("key_key.inventory".into(), "19".into())));
        assert!(!got.iter().any(|(k, _)| k == "version"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
