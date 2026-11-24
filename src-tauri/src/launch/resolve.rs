//! 读版本 JSON，沿 `inheritsFrom` 一路往上合并成一个完整的版本，并找到客户端 jar。
//!
//! 我们自己和 PCL 装的版本是合并好的独立版本；官方启动器、HMCL 装的加载器版本只写了差异，
//! 靠 `inheritsFrom` 指向原版。两种都要能启动。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::game::mojang::lib_key;

pub struct Resolved {
    /// 合并后的版本 JSON
    pub json: Value,
    /// 客户端 jar 应该在的位置（可能还没下载）
    pub jar: PathBuf,
    /// jar 对应的版本 id（下载时用）
    pub jar_id: String,
}

fn read(root: &Path, id: &str) -> Result<Value> {
    crate::instance::check_name(id)?;
    let p = root.join("versions").join(id).join(format!("{}.json", id));
    let text = std::fs::read_to_string(&p).with_context(|| format!("找不到版本文件：{}", p.display()))?;
    serde_json::from_str(&text).with_context(|| format!("版本文件格式不对：{}", p.display()))
}

pub fn resolve(root: &Path, name: &str) -> Result<Resolved> {
    // 从自己往上收集整条链：[自己, 父, 祖父…]
    let mut chain = vec![(name.to_string(), read(root, name)?)];
    let mut seen = HashSet::from([name.to_string()]);
    while let Some(parent) = chain.last().unwrap().1.get("inheritsFrom").and_then(|p| p.as_str()).map(String::from) {
        if !seen.insert(parent.clone()) || chain.len() > 8 {
            bail!("版本的继承关系绕成了圈：{}", parent);
        }
        let json = read(root, &parent).with_context(|| format!("「{}」依赖的原版 {} 不在这个游戏目录里，先装上它", name, parent))?;
        chain.push((parent, json));
    }

    // 从最上面的原版开始，一层层把子版本合并进去
    let mut iter = chain.iter().rev();
    let (_, top) = iter.next().unwrap();
    let mut json = top.clone();
    for (_, child) in iter {
        json = merge_child(&json, child);
    }

    // 客户端 jar：自己文件夹里有就用；否则看 jar 字段；再否则用链上最早有 jar 的那一层
    let own = root.join("versions").join(name).join(format!("{}.jar", name));
    let jar_field = json.get("jar").and_then(|j| j.as_str()).map(String::from);
    let mut candidates: Vec<String> = vec![name.to_string()];
    candidates.extend(jar_field);
    candidates.extend(chain.iter().skip(1).map(|(id, _)| id.clone()));
    let jar_id = candidates
        .iter()
        .find(|id| root.join("versions").join(id).join(format!("{}.jar", id)).is_file())
        .cloned()
        .unwrap_or_else(|| chain.last().unwrap().0.clone());
    let jar = if jar_id == name { own } else { root.join("versions").join(&jar_id).join(format!("{}.jar", jar_id)) };
    Ok(Resolved { json, jar, jar_id })
}

/// 把子版本合并到父版本上：库是子版本的在前、同名的以子版本为准；启动参数接在后面；其余字段子版本覆盖
pub fn merge_child(parent: &Value, child: &Value) -> Value {
    let mut out = parent.clone();
    let obj = out.as_object_mut().expect("version json is object");
    for (k, v) in child.as_object().into_iter().flatten() {
        match k.as_str() {
            "libraries" | "arguments" | "inheritsFrom" => {}
            _ => {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    obj.remove("inheritsFrom");

    let child_libs = child.get("libraries").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let keys: HashSet<String> = child_libs.iter().filter_map(lib_key).collect();
    let mut libs = child_libs;
    libs.extend(parent.get("libraries").and_then(|l| l.as_array()).into_iter().flatten().filter(|l| lib_key(l).map_or(true, |k| !keys.contains(&k))).cloned());
    obj.insert("libraries".into(), Value::Array(libs));

    if let Some(args) = child.get("arguments").and_then(|a| a.as_object()) {
        let mut merged = parent.get("arguments").and_then(|a| a.as_object()).cloned().unwrap_or_default();
        for key in ["game", "jvm"] {
            if let Some(extra) = args.get(key).and_then(|v| v.as_array()) {
                let mut list = merged.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                list.extend(extra.iter().cloned());
                merged.insert(key.into(), Value::Array(list));
            }
        }
        obj.insert("arguments".into(), Value::Object(merged));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merges_inherits_from_chain() {
        let root = std::env::temp_dir().join(format!("leafstarter-test-resolve-{}", std::process::id()));
        let write = |id: &str, v: Value| {
            let d = root.join("versions").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(format!("{}.json", id)), v.to_string()).unwrap();
        };
        write("1.20.1", json!({
            "id": "1.20.1", "mainClass": "net.minecraft.client.main.Main", "assetIndex": { "id": "5" },
            "libraries": [{ "name": "org.ow2.asm:asm:9.3" }, { "name": "com.mojang:brigadier:1.1.8" }],
            "arguments": { "game": ["--username", "${auth_player_name}"], "jvm": ["-cp", "${classpath}"] }
        }));
        std::fs::write(root.join("versions").join("1.20.1").join("1.20.1.jar"), b"jar").unwrap();
        write("fabric-loader-0.16.9-1.20.1", json!({
            "id": "fabric-loader-0.16.9-1.20.1", "inheritsFrom": "1.20.1", "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [{ "name": "org.ow2.asm:asm:9.6" }, { "name": "net.fabricmc:fabric-loader:0.16.9" }],
            "arguments": { "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "] }
        }));

        let r = resolve(&root, "fabric-loader-0.16.9-1.20.1").unwrap();
        assert_eq!(r.json["mainClass"], "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(r.json["assetIndex"]["id"], "5");
        assert!(r.json.get("inheritsFrom").is_none());
        let libs: Vec<&str> = r.json["libraries"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
        // 子版本的 asm 9.6 顶掉原版的 9.3
        assert_eq!(libs, ["org.ow2.asm:asm:9.6", "net.fabricmc:fabric-loader:0.16.9", "com.mojang:brigadier:1.1.8"]);
        assert_eq!(r.json["arguments"]["jvm"].as_array().unwrap().len(), 3);
        assert_eq!(r.jar_id, "1.20.1");

        // 缺父版本时给出能看懂的错误
        write("broken", json!({ "id": "broken", "inheritsFrom": "1.99" }));
        let e = resolve(&root, "broken").err().unwrap();
        assert!(format!("{:#}", e).contains("1.99"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
