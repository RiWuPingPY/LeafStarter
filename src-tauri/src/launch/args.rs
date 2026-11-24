//! 启动参数：按 rules 挑出这台电脑适用的参数，替换 `${...}` 占位符，拼 classpath。
//! 新版本（1.13+）写在 `arguments.game / arguments.jvm` 里，老版本只有一行 `minecraftArguments`。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::base::util::{join_rel, rules_allow, Coord, OsTarget};

/// 参数规则里的 features：我们只会打开「自定义窗口大小」
#[derive(Default, Clone, Copy)]
pub struct Features {
    pub custom_resolution: bool,
}

/// 带 features 的规则判定。os.arch 按 64 位 x86 算；os.version 只认「^10\\.」这种前缀写法
pub fn rules_match(rules: Option<&Value>, f: Features) -> bool {
    let Some(rules) = rules.and_then(|r| r.as_array()) else { return true };
    if rules.is_empty() {
        return true;
    }
    let os = OsTarget::current();
    let mut allowed = false;
    for rule in rules {
        let allow = rule.get("action").and_then(|a| a.as_str()) == Some("allow");
        let mut ok = true;
        if let Some(o) = rule.get("os") {
            if let Some(n) = o.get("name").and_then(|n| n.as_str()) {
                ok &= n == os.name();
            }
            if let Some(a) = o.get("arch").and_then(|a| a.as_str()) {
                ok &= a == "x86_64" || a == "amd64";
            }
            if let Some(v) = o.get("version").and_then(|v| v.as_str()) {
                ok &= os_version_matches(v);
            }
        }
        if let Some(feat) = rule.get("features").and_then(|f| f.as_object()) {
            for (k, v) in feat {
                let have = k == "has_custom_resolution" && f.custom_resolution;
                ok &= v.as_bool().unwrap_or(false) == have;
            }
        }
        if ok {
            allowed = allow;
        }
    }
    allowed
}

/// Windows 10 和 11 报的版本号都是 10.0
fn os_version_matches(pattern: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let prefix = pattern.trim_start_matches('^').replace("\\.", ".");
    "10.0".starts_with(&prefix)
}

/// 把 `arguments.game` / `arguments.jvm` 展开成字符串列表（还没替换占位符）
pub fn collect(list: Option<&Value>, f: Features) -> Vec<String> {
    let mut out = vec![];
    for item in list.and_then(|l| l.as_array()).into_iter().flatten() {
        match item {
            Value::String(s) => out.push(s.clone()),
            Value::Object(_) => {
                if !rules_match(item.get("rules"), f) {
                    continue;
                }
                match item.get("value") {
                    Some(Value::String(s)) => out.push(s.clone()),
                    Some(Value::Array(a)) => out.extend(a.iter().filter_map(|v| v.as_str()).map(String::from)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

/// 替换 `${名字}`；不认识的占位符原样留着
pub fn fill(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        match rest[start..].find('}') {
            Some(end) => {
                let key = &rest[start + 2..start + end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + end + 1]),
                }
                rest = &rest[start + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// classpath：这台电脑适用的库（不含老式的 natives 包）加上客户端 jar，按出现顺序去重
pub fn classpath(json: &Value, libs_dir: &Path, jar: &Path) -> Vec<PathBuf> {
    let os = OsTarget::current();
    let mut seen = HashSet::new();
    let mut out = vec![];
    for lib in json.get("libraries").and_then(|l| l.as_array()).into_iter().flatten() {
        if !rules_allow(lib.get("rules"), os) || lib.get("clientreq").and_then(|v| v.as_bool()) == Some(false) {
            continue;
        }
        let name = lib.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let art = lib.get("downloads").and_then(|d| d.get("artifact"));
        // 只有 natives、没有主 jar 的老式条目（比如 lwjgl-platform）不进 classpath
        if lib.get("natives").is_some() && art.is_none() {
            continue;
        }
        let rel = art.and_then(|a| a.get("path")).and_then(|p| p.as_str()).map(String::from).or_else(|| Coord::parse(name).map(|c| c.path()));
        if let Some(rel) = rel {
            let p = join_rel(libs_dir, &rel);
            if seen.insert(p.clone()) {
                out.push(p);
            }
        }
    }
    if seen.insert(jar.to_path_buf()) {
        out.push(jar.to_path_buf());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fills_placeholders() {
        let vars = HashMap::from([("auth_player_name", "Steve".to_string()), ("version_name", "1.20.1".to_string())]);
        assert_eq!(fill("--username ${auth_player_name}", &vars), "--username Steve");
        assert_eq!(fill("${version_name}-${unknown}", &vars), "1.20.1-${unknown}");
        assert_eq!(fill("no vars", &vars), "no vars");
        assert_eq!(fill("broken ${oops", &vars), "broken ${oops");
    }

    #[test]
    fn evaluates_rules_and_features() {
        let args = json!([
            "--username", "${auth_player_name}",
            { "rules": [{ "action": "allow", "features": { "is_demo_user": true } }], "value": "--demo" },
            { "rules": [{ "action": "allow", "features": { "has_custom_resolution": true } }], "value": ["--width", "${resolution_width}"] },
            { "rules": [{ "action": "allow", "os": { "name": "windows" } }], "value": "-XX:HeapDumpPath=x" },
            { "rules": [{ "action": "allow", "os": { "arch": "x86" } }], "value": "-Xss1M" }
        ]);
        let off = collect(Some(&args), Features::default());
        let on = collect(Some(&args), Features { custom_resolution: true });
        assert!(!off.contains(&"--demo".to_string()) && !off.contains(&"--width".to_string()));
        assert!(on.contains(&"--width".to_string()));
        assert!(!off.contains(&"-Xss1M".to_string()));
        assert_eq!(off.contains(&"-XX:HeapDumpPath=x".to_string()), cfg!(windows));
    }

    #[test]
    fn builds_classpath() {
        let v = json!({ "libraries": [
            { "name": "org.ow2.asm:asm:9.6" },
            { "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4", "natives": { "windows": "natives-windows" } },
            { "name": "org.ow2.asm:asm:9.6" },
            { "name": "x:only-mac:1", "rules": [{ "action": "allow", "os": { "name": "osx" } }] },
            { "name": "com.mojang:brigadier:1.1.8", "downloads": { "artifact": { "path": "com/mojang/brigadier/1.1.8/brigadier-1.1.8.jar" } } }
        ] });
        let cp = classpath(&v, Path::new("L"), Path::new("game.jar"));
        let names: Vec<String> = cp.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        // 重复的 asm 只留一个；只有 natives 的 lwjgl-platform 和只给 macOS 的库都不进 classpath
        if !cfg!(target_os = "macos") {
            assert_eq!(names, ["asm-9.6.jar", "brigadier-1.1.8.jar", "game.jar"]);
        }
    }
}
