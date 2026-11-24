//! 从方块补给搬设置过来。
//!
//! D2 换了应用标识（`com.leafhub.mcsupply` → `com.leafhub.leafstarter`），数据目录也跟着换了。
//! 第一次打开 LeafStarter 时，如果新目录里还没有设置，就把方块补给的 `settings.json`
//! （下载源设置、手动添加的游戏目录）复制过来；方块补给那边的文件不动，两个软件可以同时装。
//!
//! 只在桌面端做：安卓上读不到别的应用的私有目录，目录授权也跟着应用走，搬不过来。

use std::path::{Path, PathBuf};

pub const OLD_IDENTIFIER: &str = "com.leafhub.mcsupply";

/// 搬了就返回方块补给那份设置的路径；不需要搬或者搬不了返回 None。
pub fn import_mc_supply(data_dir: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "android") {
        return None;
    }
    import_from(data_dir, &data_dir.parent()?.join(OLD_IDENTIFIER))
}

fn import_from(data_dir: &Path, old_dir: &Path) -> Option<PathBuf> {
    let dest = data_dir.join("settings.json");
    let src = old_dir.join("settings.json");
    if dest.exists() || !src.is_file() {
        return None;
    }
    std::fs::create_dir_all(data_dir).ok()?;
    std::fs::copy(&src, &dest).ok()?;
    Some(src)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("leafstarter-test-{}-{}-{}", name, std::process::id(), nanos));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn copies_once_and_never_overwrites() {
        let root = scratch("migrate");
        let old = root.join(OLD_IDENTIFIER);
        let new = root.join("com.leafhub.leafstarter");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("settings.json"), r#"{"manualDirs":["D:\\mc"]}"#).unwrap();

        // 第一次：搬过来，内容一致
        assert_eq!(import_mc_supply(&new), Some(old.join("settings.json")));
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), r#"{"manualDirs":["D:\\mc"]}"#);

        // 之后：新目录已经有设置了，不再搬，也不覆盖
        std::fs::write(new.join("settings.json"), "{}").unwrap();
        assert_eq!(import_mc_supply(&new), None);
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "{}");

        // 方块补给的文件原样保留
        assert!(old.join("settings.json").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_to_import() {
        let root = scratch("migrate-empty");
        assert_eq!(import_mc_supply(&root.join("com.leafhub.leafstarter")), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
