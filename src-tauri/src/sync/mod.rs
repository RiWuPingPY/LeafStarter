//! 手机电脑同一个世界（ROADMAP 5.7）。
//!
//! - `store`：本机的文件仓库（按 SHA-1 去重）和存档历史
//! - `crypto`、`devices`：局域网直传的加密和配对过的设备
//! - `lan`：服务端（让别的设备连过来）；`peer`：连到别的设备；`engine`：拉实例、拉世界、推世界
//! - 以后：`webdav`（自己的网盘）、LF 云（M18）用同一套清单和仓库

pub mod crypto;
pub mod devices;
pub mod engine;
pub mod lan;
pub mod peer;
pub mod store;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use super::devices::Devices;
    use super::engine::{self, Outcome};
    use super::lan::{Ctx, Host, RemoteInst, Server};
    use super::peer::{self, Client};
    use super::store::Store;

    struct TestHost(PathBuf);
    impl Host for TestHost {
        fn instances(&self) -> Vec<RemoteInst> {
            vec![RemoteInst { key: "d/inst".into(), name: "inst".into(), mc: Some("1.20.1".into()), loader: "Fabric".into(), loader_version: None, launcher: "测试".into(), worlds: vec![] }]
        }
        fn game_dir(&self, key: &str) -> Option<PathBuf> {
            (key == "d/inst").then(|| self.0.clone())
        }
        fn running(&self, _: &str) -> bool {
            false
        }
    }

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn pair_pull_push_conflict() {
        let base = std::env::temp_dir().join(format!("ls-lan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        // B 的游戏目录放在另一个盘上（仓库在系统盘、游戏在 D 盘很常见），测跨盘写文件
        let other = std::path::PathBuf::from("D:/Document/LeafStarter-build/tmp-lan-test");
        let b_game_root = if other.parent().is_some_and(|p| p.is_dir()) { other } else { base.clone() };
        let (a_data, a_gd, b_data, b_gd) = (base.join("a-data"), base.join("a-game"), base.join("b-data"), b_game_root.join("b-game"));
        let _ = std::fs::remove_dir_all(&b_gd);
        write(&a_gd.join("mods/sodium.jar"), "sodium");
        write(&a_gd.join("config/sodium.json"), "{}");
        write(&a_gd.join("options.txt"), "fov:0.5");
        write(&a_gd.join("saves/w/level.dat"), "a1");
        write(&a_gd.join("saves/w/region/r.0.0.mca"), "region");
        write(&b_gd.join("mods/old.jar"), "old");
        write(&b_gd.join("config/mine.json"), "keep");

        tauri::async_runtime::block_on(async {
            let a_store = Arc::new(Store::open(&a_data));
            let ctx = Arc::new(Ctx::new(a_data.clone(), a_store.clone(), Arc::new(TestHost(a_gd.clone()))));
            let srv = Server::start(ctx.clone(), "127.0.0.1".parse().unwrap()).await.unwrap();
            let token = ctx.new_pair_token();
            let me_a = ctx.devices.lock().unwrap().me.clone();
            let code = peer::pair_code(&["127.0.0.1".into()], srv.port, &token, &me_a);

            // 配对：口令只能用一次
            let me_b = Devices::load(&b_data).me;
            let known = peer::pair(&code, &me_b).await.unwrap();
            assert!(peer::pair(&code, &me_b).await.is_err(), "口令用过就作废");
            let c = Client::new(&me_b, &known).unwrap();
            let (info, _) = c.call("/info", serde_json::json!({}), &[]).await.unwrap();
            assert_eq!(info["instances"][0]["key"], "d/inst");

            // 拉实例：mods 和对方一致（old.jar 删掉），config 只加不删
            let b_store = Store::open(&b_data);
            let quiet = |_: String| {};
            engine::pull_instance(&c, &b_store, "d/inst", &b_gd, &quiet).await.unwrap();
            assert!(b_gd.join("mods/sodium.jar").is_file());
            assert!(!b_gd.join("mods/old.jar").exists());
            assert!(b_gd.join("config/mine.json").is_file());
            assert_eq!(std::fs::read_to_string(b_gd.join("options.txt")).unwrap(), "fov:0.5");

            // 拉世界，在 B 上玩一会儿再推回去
            let (o, base1) = engine::pull_world(&c, &b_store, "d/inst", "w", &b_gd, None, false, "k", &quiet).await.unwrap();
            assert_eq!(o, Outcome::Done { files: 2 });
            write(&b_gd.join("saves/w/level.dat"), "b2");
            let (o, base2) = engine::push_world(&c, &b_store, "d/inst", "w", &b_gd, Some(&base1), false, &quiet).await.unwrap();
            assert_eq!(o, Outcome::Done { files: 1 }, "只传变了的 level.dat");
            assert_eq!(std::fs::read_to_string(a_gd.join("saves/w/level.dat")).unwrap(), "b2");
            // 对方那份旧的进了历史
            let hk = crate::commands::sync::world_key(&a_gd, "w");
            assert!(a_store.list(&hk).iter().any(|s| s.reason == "before-sync"));

            // 两边都改：冲突，不覆盖
            write(&a_gd.join("saves/w/level.dat"), "a3");
            write(&b_gd.join("saves/w/level.dat"), "b3");
            let (o, _) = engine::push_world(&c, &b_store, "d/inst", "w", &b_gd, Some(&base2), false, &quiet).await.unwrap();
            assert_eq!(o, Outcome::Conflict);
            assert_eq!(std::fs::read_to_string(a_gd.join("saves/w/level.dat")).unwrap(), "a3");
            let (o, _) = engine::pull_world(&c, &b_store, "d/inst", "w", &b_gd, Some(&base2), false, "k", &quiet).await.unwrap();
            assert_eq!(o, Outcome::Conflict);
            // 用户选「要对方的」
            let (o, _) = engine::pull_world(&c, &b_store, "d/inst", "w", &b_gd, Some(&base2), true, "k", &quiet).await.unwrap();
            assert_eq!(o, Outcome::Done { files: 1 });
            assert_eq!(std::fs::read_to_string(b_gd.join("saves/w/level.dat")).unwrap(), "a3");
            assert!(b_store.list("k").iter().any(|s| s.reason == "before-sync"), "B 自己那份 b3 进了历史");

            // 删掉配对后就不认了
            ctx.devices.lock().unwrap().clients.clear();
            assert!(c.call("/info", serde_json::json!({}), &[]).await.is_err());
            drop(srv);
        });
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&b_gd);
    }
}
