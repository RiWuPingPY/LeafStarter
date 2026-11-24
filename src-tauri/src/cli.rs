//! 调试 / 高级用法：`leafstarter --cli <命令>`，不开窗口直接跑。

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::base::net::{Net, NetSettings};
use crate::base::task::Tasks;
use crate::base::util;
use crate::game::install::{self, Env, InstallRequest};
use crate::game::java;
use crate::game::launchers::{self, Target};
use crate::game::loaders::{self, LoaderKind};

/// 返回 None 表示不是命令行模式。
pub fn cli(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--cli") {
        return None;
    }
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().ok()?;
    let code = rt.block_on(async {
        let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
        let net = Arc::new(Net::new(NetSettings::default()).expect("http client"));
        let data_dir = std::env::temp_dir().join("leafstarter-cli");
        match args.get(2).map(String::as_str) {
            Some("sources") => {
                net.speed_test().await;
                println!("{}", serde_json::to_string_pretty(&net.views()).unwrap());
                0
            }
            Some("scan") => {
                #[cfg(not(target_os = "android"))]
                for t in launchers::scan_desktop(&[]) {
                    println!("{:<14} {:<4} {}  ({} 个版本)", t.launcher, t.access, t.path, t.versions.len());
                }
                0
            }
            Some("java") => {
                for j in java::detect(&data_dir) {
                    println!("Java {:<3} {:<12} {}  [{}]", j.major, j.version, j.path, j.source);
                }
                0
            }
            Some("loaders") => {
                let kind = get("--loader").and_then(|k| LoaderKind::parse(&k)).unwrap_or(LoaderKind::Fabric);
                let mc = get("--mc").unwrap_or_else(|| "1.20.1".into());
                match loaders::list(&net, kind, &mc).await {
                    Ok(list) => {
                        for v in list.iter().take(15) {
                            println!("{:<16} {:<6} {}", v.version, v.tag.clone().unwrap_or_default(), v.id);
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("{:#}", e);
                        1
                    }
                }
            }
            Some("install") => {
                let dir = get("--dir").expect("--dir <.minecraft 目录>");
                let mc = get("--mc").expect("--mc <版本>");
                net.speed_test().await;
                let loader = match (get("--loader"), get("--loader-version")) {
                    (Some(kind), Some(ver)) => {
                        let k = LoaderKind::parse(&kind).expect("loader");
                        let list = loaders::list(&net, k, &mc).await.expect("loader list");
                        let v = list.into_iter().find(|v| v.version == ver).expect("找不到这个加载器版本");
                        Some(install::LoaderReq { kind: k.label().into(), id: v.id, version: v.version })
                    }
                    _ => None,
                };
                let name = get("--name").unwrap_or_else(|| match &loader {
                    Some(l) => format!("{}-{} {}", mc, l.kind, l.version),
                    None => mc.clone(),
                });
                let target = Target {
                    id: launchers::target_id(&dir),
                    kind: "custom".into(),
                    launcher: "命令行".into(),
                    path: dir.clone(),
                    access: "direct".into(),
                    exe: None,
                    versions: vec![],
                    manual: true,
                    saf_uri: None,
                    saf_rel: None,
                    package: None,
                    hint: None,
                };
                let tasks = Tasks::default();
                let task = tasks.create(name.clone(), String::new(), dir.clone());
                let env = Env { app: None, net: net.clone(), data_dir };
                let req = InstallRequest { target_id: target.id.clone(), mc_version: mc, name, loader, overwrite: true };
                let t2 = task.clone();
                let ticker = tokio::spawn(async move {
                    let mut last = String::new();
                    loop {
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        let s = t2.snapshot();
                        let line = format!("{} [{}/{} 文件 {}]", s.stage, s.files_done, s.files_total, util::human_size(s.bytes_done));
                        if line != last {
                            println!("{}", line);
                            last = line;
                        }
                    }
                });
                let t0 = Instant::now();
                let r = install::run(&env, target, req, task.clone()).await;
                ticker.abort();
                let s = task.snapshot();
                println!("各源下载量：{:?}", s.sources.iter().map(|(k, v)| format!("{}={}", k, util::human_size(*v))).collect::<Vec<_>>());
                match r {
                    Ok(p) => {
                        println!("完成，用时 {:.1}s：{}", t0.elapsed().as_secs_f64(), p.unwrap_or_default());
                        if let Some(n) = s.note {
                            println!("{}", n);
                        }
                        0
                    }
                    Err(e) => {
                        eprintln!("失败：{:#}", e);
                        1
                    }
                }
            }
            _ => {
                eprintln!("用法：leafstarter --cli <sources|scan|java|loaders|install> [--mc 版本] [--loader 类型] [--loader-version 版本] [--dir 目录]");
                2
            }
        }
    });
    Some(code)
}
