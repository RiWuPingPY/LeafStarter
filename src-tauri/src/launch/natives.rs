//! 本地库（natives）：启动前解压到 natives 目录，游戏通过 `-Djava.library.path` 找到它们。
//!
//! 两种写法：1.18 及更早用 `natives` 分类器（整包解压，跳过 META-INF）；
//! 1.19 起本地库就是普通的库，名字带 `:natives-windows`，只取里面的 .dll。

use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::base::util::{join_rel, rules_allow, Coord, OsTarget};

const NATIVE_EXT: [&str; 3] = [".dll", ".so", ".dylib"];

pub fn extract(json: &Value, libs_dir: &Path, natives_dir: &Path) -> Result<usize> {
    let os = OsTarget::current();
    // 每次清空重来，免得留着别的版本的旧文件
    let _ = std::fs::remove_dir_all(natives_dir);
    std::fs::create_dir_all(natives_dir).with_context(|| format!("建不了 natives 目录：{}", natives_dir.display()))?;
    let mut count = 0;
    for lib in json.get("libraries").and_then(|l| l.as_array()).into_iter().flatten() {
        if !rules_allow(lib.get("rules"), os) {
            continue;
        }
        let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if let Some(classifier) = lib.get("natives").and_then(|n| n.get(os.name())).and_then(|c| c.as_str()) {
            // 老式：整包解压
            let classifier = classifier.replace("${arch}", "64");
            let rel = lib
                .get("downloads")
                .and_then(|d| d.get("classifiers"))
                .and_then(|c| c.get(&classifier))
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
                .map(String::from)
                .or_else(|| {
                    Coord::parse(name).map(|mut c| {
                        c.classifier = Some(classifier.clone());
                        c.path()
                    })
                });
            let Some(rel) = rel else { continue };
            let exclude: Vec<String> = lib
                .get("extract")
                .and_then(|e| e.get("exclude"))
                .and_then(|e| e.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).map(String::from).collect())
                .unwrap_or_else(|| vec!["META-INF/".into()]);
            count += unzip(&join_rel(libs_dir, &rel), natives_dir, |entry| !exclude.iter().any(|x| entry.starts_with(x.as_str())), false)?;
        } else if Coord::parse(name).and_then(|c| c.classifier).is_some_and(|c| c == format!("natives-{}", os.name())) {
            // 新式：只取本地库文件，拍平到 natives 目录
            let rel = lib
                .get("downloads")
                .and_then(|d| d.get("artifact"))
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
                .map(String::from)
                .or_else(|| Coord::parse(name).map(|c| c.path()));
            let Some(rel) = rel else { continue };
            count += unzip(&join_rel(libs_dir, &rel), natives_dir, |entry| NATIVE_EXT.iter().any(|e| entry.to_ascii_lowercase().ends_with(e)), true)?;
        }
    }
    Ok(count)
}

fn unzip(jar: &Path, dest: &Path, keep: impl Fn(&str) -> bool, flatten: bool) -> Result<usize> {
    let file = std::fs::File::open(jar).with_context(|| format!("找不到本地库：{}", jar.display()))?;
    let mut zip = zip::ZipArchive::new(file).with_context(|| format!("本地库不是有效的 jar：{}", jar.display()))?;
    let mut n = 0;
    for k in 0..zip.len() {
        let mut entry = zip.by_index(k)?;
        let name = entry.name().to_string();
        if entry.is_dir() || !keep(&name) {
            continue;
        }
        // 防止 ../ 跑出目录
        let Some(safe) = entry.enclosed_name() else { continue };
        let out = if flatten { dest.join(safe.file_name().unwrap_or_default()) } else { dest.join(safe) };
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&out, buf).with_context(|| format!("写不了 {}", out.display()))?;
        n += 1;
    }
    Ok(n)
}
