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
            // 整合包：pack --file <.mrpack|zip> 或 --modrinth <slug>，装进 --dir 成为 --name；export 导出成 --out
            Some(cmd @ ("pack" | "export")) => {
                let dir = get("--dir").expect("--dir <.minecraft 目录>");
                let name = get("--name").expect("--name <实例名>");
                #[cfg(not(target_os = "android"))]
                let targets = launchers::scan_desktop(std::slice::from_ref(&dir));
                #[cfg(target_os = "android")]
                let targets: Vec<Target> = vec![];
                let norm = |p: &str| p.replace('/', "\\").trim_end_matches('\\').to_lowercase();
                let Some(target) = targets.into_iter().find(|t| norm(&t.path) == norm(&dir)) else {
                    eprintln!("找不到游戏目录 {dir}");
                    return 1;
                };
                if cmd == "export" {
                    let out = std::path::PathBuf::from(get("--out").expect("--out <文件>"));
                    let Some(v) = target.versions.iter().find(|v| v.name == name) else {
                        eprintln!("找不到实例 {name}");
                        return 1;
                    };
                    let gd = crate::instance::game_dir(std::path::Path::new(&target.path), &name).0;
                    let lv = v.loader_version.clone().unwrap_or_default();
                    let loader = (!v.loader.is_empty()).then_some((v.loader.as_str(), lv.as_str()));
                    return match crate::content::export::export(&gd, &out, &name, "1.0.0", v.mc.as_deref().unwrap_or(""), loader, false).await {
                        Ok(s) => {
                            println!("导出好了：{}（{} 个只写地址，{} 个打包进去）", out.display(), s.linked, s.bundled);
                            0
                        }
                        Err(e) => {
                            eprintln!("导出失败：{e:#}");
                            1
                        }
                    };
                }
                net.speed_test().await;
                let file = match (get("--file"), get("--modrinth")) {
                    (Some(f), _) => std::path::PathBuf::from(f),
                    (None, Some(slug)) => {
                        let http = crate::content::modrinth::client().expect("http");
                        let list = crate::content::modrinth::versions(&http, &slug, None, None).await.unwrap_or_default();
                        let Some(f) = crate::content::modrinth::pick(&list).and_then(|v| v.primary().cloned()) else {
                            eprintln!("Modrinth 上找不到 {slug} 的整合包文件");
                            return 1;
                        };
                        println!("下载 {}（{}）", f.filename, util::human_size(f.size));
                        let p = data_dir.join("packs").join(&f.filename);
                        let job = crate::base::net::FileJob { url: f.url, path: p.clone(), sha1: Some(f.sha1), size: Some(f.size) };
                        if let Err(e) = install::download_all(&net, &[job], &Arc::new(crate::base::net::Progress::default())).await {
                            eprintln!("下载失败：{e:#}");
                            return 1;
                        }
                        p
                    }
                    _ => {
                        eprintln!("要 --file <整合包> 或者 --modrinth <slug>");
                        return 2;
                    }
                };
                let plan = match crate::content::pack::read(&file).await {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("读不了整合包：{e:#}");
                        return 1;
                    }
                };
                println!("{}（{}）：{} {:?}，{} 个文件，{} 个要手动下", plan.name, plan.format, plan.mc, plan.loader, plan.files.len(), plan.manual.len());
                let tasks = Tasks::default();
                let task = tasks.create(name.clone(), String::new(), dir.clone());
                let env = Env { app: None, net: net.clone(), data_dir };
                let t0 = Instant::now();
                match crate::content::pack::install(&env, target, &plan, &file, &name, task).await {
                    Ok(m) => {
                        println!("完成，用时 {:.1}s：{}", t0.elapsed().as_secs_f64(), m.unwrap_or_default());
                        0
                    }
                    Err(e) => {
                        eprintln!("失败：{e:#}");
                        1
                    }
                }
            }
            Some("mods") => {
                let prefer = get("--loader").unwrap_or_default();
                for d in args.iter().skip(3).filter(|a| !a.starts_with("--") && Some(a.as_str()) != get("--loader").as_deref()) {
                    let all = crate::game::modmeta::read_dir(std::path::Path::new(d), &prefer);
                    let known = all.iter().filter(|m| m.id.is_some()).count();
                    println!("{d}: {} 个文件，认出 {known} 个", all.len());
                    for (f, dep) in crate::game::modmeta::missing_deps(&all, &prefer) {
                        println!("  缺前置 {dep:<28} ← {f}");
                    }
                }
                0
            }
            Some("world") => {
                for d in args.iter().skip(3) {
                    match crate::game::world::info(std::path::Path::new(d)) {
                        Ok(mut w) => {
                            w.icon = w.icon.map(|i| format!("{} 字节", i.len()));
                            println!("{}", serde_json::to_string(&w).unwrap());
                        }
                        Err(e) => println!("{d}: {e:#}"),
                    }
                }
                0
            }
            Some("unixsock") => {
                for d in args.iter().skip(3) {
                    println!("{:<70} {}", d, if crate::launch::unixsock::works(std::path::Path::new(d)) { "能用" } else { "不能用" });
                }
                0
            }
            // 调试启动：args 只打印 Java 和完整参数；launch 真的启动（离线账户），把日志打出来；
            // smoke 是验收用的：启动到资源加载完（主菜单出来前后）就结束游戏，打印用时，没到就算失败
            Some(cmd @ ("args" | "launch" | "smoke")) => {
                let root = std::path::PathBuf::from(get("--dir").expect("--dir <.minecraft 目录>"));
                let name = get("--name").expect("--name <版本名>");
                let user = get("--user").unwrap_or_else(|| "Steve".into());
                let auth = crate::auth::LaunchAuth { uuid: crate::auth::offline_uuid(&user), name: user, token: "0".into(), user_type: "legacy", ygg: None };
                let report: crate::launch::Report = Arc::new(|phase: &str, pct: u32, detail: String| println!("[{}] {}% {}", phase, pct, detail));
                match crate::launch::prepare(&net, &data_dir, &root, &name, &auth, &crate::launch::Options::default(), report).await {
                    Ok(p) if cmd == "args" => {
                        println!("java: {} (Java {})", p.java.display(), p.java_major);
                        for a in &p.args {
                            println!("  {}", a);
                        }
                        0
                    }
                    Ok(p) if cmd == "smoke" => match crate::launch::process::smoke(&p.java, &p.args, p.java_major, &p.game_dir, &p.natives_dir, Duration::from_secs(300), None, None).await {
                        Ok(r) if r.ok => {
                            println!("SMOKE OK {:.1}s Java {} :: {}", r.secs, p.java_major, r.hit.unwrap_or_default());
                            0
                        }
                        Ok(r) => {
                            println!("SMOKE FAIL {:.1}s Java {} 退出码 {:?}{}", r.secs, p.java_major, r.code, if r.timed_out { "（超时）" } else { "" });
                            for l in r.tail.iter().rev().take(40).rev() {
                                println!("  | {}", l);
                            }
                            1
                        }
                        Err(e) => {
                            eprintln!("{:#}", e);
                            1
                        }
                    },
                    Ok(p) => {
                        let (_keep, rx) = tokio::sync::oneshot::channel::<()>();
                        let on_line: Arc<dyn Fn(String) + Send + Sync> = Arc::new(|l: String| println!("{}", l));
                        match crate::launch::process::run(&p.java, &p.args, p.java_major, &p.game_dir, &p.natives_dir, rx, on_line).await {
                            Ok(exit) => {
                                println!("游戏退出，退出码 {:?}", exit.code);
                                exit.code.unwrap_or(1)
                            }
                            Err(e) => {
                                eprintln!("{:#}", e);
                                1
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("没能启动：{:#}", e);
                        1
                    }
                }
            }
            _ => {
                eprintln!("用法：leafstarter --cli <sources|scan|java|loaders|install|args|launch|smoke> [--mc 版本] [--loader 类型] [--loader-version 版本] [--dir 目录]");
                2
            }
        }
    });
    Some(code)
}

