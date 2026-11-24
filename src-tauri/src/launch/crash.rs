//! 崩溃分析：先用规则认（快、免费、不用联网），认出来的给出原因和一键修复。
//! 规则认不出的，以后交给 AI 读日志（M9）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Fix {
    /// 把这个实例的内存改成多少 MB
    Memory { mb: u32 },
    /// 不再指定 Java，按版本要求自动挑
    JavaAuto,
    /// 换成至少这个大版本的 Java（本机没有就下载官方运行时）
    JavaMin { major: u32 },
    /// 禁用这几个 Mod 文件
    DisableMods { files: Vec<String> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub title: String,
    pub detail: String,
    pub fix: Option<Fix>,
    pub fix_label: String,
}

pub struct Ctx<'a> {
    pub memory_mb: u32,
    pub total_mb: u64,
    pub java_major: u32,
    /// 用户在实例设置里自己指定了 Java
    pub java_custom: bool,
    /// 启用着的 Mod 文件名
    pub mods: &'a [String],
}

fn diag(title: impl Into<String>, detail: impl Into<String>, fix: Option<Fix>, label: impl Into<String>) -> Option<Diagnosis> {
    Some(Diagnosis { title: title.into(), detail: detail.into(), fix, fix_label: label.into() })
}

/// 在 text 里找 `前缀` 后面紧跟的数字
fn number_after(text: &str, prefix: &str) -> Option<u32> {
    let i = text.find(prefix)? + prefix.len();
    text[i..].trim_start().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// 找名字里含 key 的 Mod 文件（忽略大小写、连字符和下划线）
fn mod_files_like(mods: &[String], key: &str) -> Vec<String> {
    let norm = |s: &str| s.to_ascii_lowercase().replace(['-', '_', ' '], "");
    let k = norm(key);
    if k.len() < 3 {
        return vec![];
    }
    mods.iter().filter(|m| norm(m).contains(&k)).cloned().collect()
}

pub fn analyze(log: &[String], report: &str, ctx: &Ctx) -> Option<Diagnosis> {
    let tail = log.iter().rev().take(3000).rev().cloned().collect::<Vec<_>>().join("\n");
    let text = format!("{}\n{}", report, tail);
    let has = |s: &str| text.contains(s);

    // Java 本身坏了：下到一半的运行时、被杀毒软件删了文件
    if has("Error occurred during initialization of VM") && (has("java/lang/Object") || has("java.lang.Object") || has("Could not find or load main class java")) {
        return diag(
            "这个 Java 坏了：缺了自带的类库文件",
            "常见原因是 Java 没下载完就被打断了，或者杀毒软件删了它的文件。换一个 Java，或者删掉这个 Java 的文件夹让启动器重新下载。",
            ctx.java_custom.then_some(Fix::JavaAuto),
            "改回自动选 Java 并重新启动",
        );
    }
    // Java 太旧：class file version 65.0 = Java 21
    if has("UnsupportedClassVersionError") || has("compiled by a more recent version of the Java Runtime") {
        if let Some(v) = number_after(&text, "class file version") {
            let need = v.saturating_sub(44);
            if need > ctx.java_major {
                return diag(
                    format!("Java 版本太旧：要 Java {}，现在用的是 Java {}", need, ctx.java_major),
                    "游戏本身或者某个 Mod 是用更新的 Java 编译的，旧的 Java 打不开它。",
                    Some(Fix::JavaMin { major: need }),
                    format!("换成 Java {} 并重新启动", need),
                );
            }
        }
    }
    // Java 太新：老版本 Forge 用的 ASM 读不了新 Java 的类
    if has("Unsupported class file major version") || (has("cpw.mods.modlauncher") && has("Unsupported major.minor")) {
        return diag(
            "Java 太新了：这个版本要更老的 Java",
            "老版本的 Forge 读不懂新版 Java 的类文件。按版本要求自动挑 Java 就好。",
            ctx.java_custom.then_some(Fix::JavaAuto),
            "改回自动选 Java 并重新启动",
        );
    }
    if has("java.lang.OutOfMemoryError") {
        let target = ((ctx.memory_mb as u64 + 2048).min(ctx.total_mb * 3 / 4) / 512 * 512) as u32;
        let fix = (target > ctx.memory_mb).then_some(Fix::Memory { mb: target });
        return diag(
            format!("内存不够：分给游戏的 {} MB 用完了", ctx.memory_mb),
            if fix.is_some() { "Mod 多、光影或视距开得大时很常见。多给一点内存再试。" } else { "已经分了本机内存的大部分了。可以关掉光影、调小视距，或者少装几个 Mod。" },
            fix,
            format!("内存改成 {} MB 并重新启动", target),
        );
    }
    if has("Could not reserve enough space for") || has("Invalid maximum heap size") {
        let target = ((ctx.total_mb / 3).clamp(1024, 4096) / 512 * 512) as u32;
        return diag(
            "分配的内存太大，Java 申请不下来",
            "可能是用了 32 位的 Java，或者本机剩余内存不够。",
            Some(if ctx.java_custom { Fix::JavaAuto } else { Fix::Memory { mb: target } }),
            if ctx.java_custom { "改回自动选 Java 并重新启动".to_string() } else { format!("内存改成 {} MB 并重新启动", target) },
        );
    }
    // 重复的 Mod（Forge：from mod files: a.jar, b.jar）
    if let Some(i) = text.find("from mod files:") {
        let line = text[i + 15..].lines().next().unwrap_or("");
        let mut files: Vec<String> = line.split(',').map(|f| f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()).filter(|f| f.ends_with(".jar")).collect();
        if files.len() >= 2 {
            files.sort();
            let keep = files.pop().unwrap();
            return diag(
                "同一个 Mod 装了两份",
                format!("{} 和 {} 是同一个 Mod 的不同文件。留下 {}（看起来更新），禁用其余的。", files.join("、"), keep, keep),
                Some(Fix::DisableMods { files }),
                "禁用重复的并重新启动",
            );
        }
    }
    if has("Duplicate mods found") || has("is provided by multiple mods") || has("Found duplicate mod") {
        return diag("同一个 Mod 装了两份", "日志里报了重复的 Mod。到「Mod」里找名字一样的，禁用旧的那个。", None, "");
    }
    // Fabric：缺前置或者版本对不上
    if has("Incompatible mod set") || has("incompatible mod set") || has("which is missing!") {
        let lines: Vec<String> = text.lines().filter(|l| l.contains("requires") || l.contains("which is missing") || l.contains("is incompatible with")).map(|l| l.trim().trim_start_matches("- ").to_string()).take(8).collect();
        return diag("Mod 之间对不上：缺前置，或者版本不对", if lines.is_empty() { "Fabric 在加载前检查到 Mod 之间不兼容，详细的看日志。".to_string() } else { lines.join("\n") }, None, "");
    }
    // Forge：缺前置
    if has("Missing or unsupported mandatory dependencies") {
        let lines: Vec<String> = text
            .lines()
            .filter(|l| l.contains("Mod ID:") && l.contains("Requested by"))
            .map(|l| {
                let q = |k: &str| l.find(k).map(|i| l[i + k.len()..].split('\'').nth(1).unwrap_or("").to_string()).unwrap_or_default();
                let actual = q("Actual version:");
                format!("「{}」需要「{}」{}，{}", q("Requested by:"), q("Mod ID:"), q("Expected range:"), if actual == "[MISSING]" || actual.is_empty() { "但没装".to_string() } else { format!("装的是 {}", actual) })
            })
            .take(8)
            .collect();
        return diag("缺前置 Mod，或者前置的版本不对", if lines.is_empty() { "Forge 在加载前检查到缺少必需的 Mod，详细的看日志。".to_string() } else { lines.join("\n") }, None, "");
    }
    // Mixin 注入失败：通常是某个 Mod 和游戏版本或别的 Mod 不兼容
    for pat in ["Mixin apply for mod ", "Mixin apply failed ", "from mod "] {
        if let Some(i) = text.find(pat) {
            if !(has("Mixin") && (has("MixinApplyError") || has("Mixin apply") || has("InvalidInjectionException") || has("MixinTransformerError"))) {
                break;
            }
            let id: String = text[i + pat.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
            let id = id.split(".mixins").next().unwrap_or(&id).to_string();
            let files = mod_files_like(ctx.mods, &id);
            return diag(
                format!("Mod「{}」改游戏代码的时候失败了", id),
                "多半是这个 Mod 和现在的游戏版本、或者和别的 Mod 不兼容。先禁用它，能进游戏就说明是它；再去找它适配这个版本的新版本。",
                (!files.is_empty()).then(|| Fix::DisableMods { files: files.clone() }),
                format!("先禁用 {} 试试", files.first().cloned().unwrap_or_default()),
            );
        }
    }
    if has("Pixel format not accelerated") || has("WGL: The driver does not appear to support OpenGL") || has("GLFW error 65542") || has("GLFW error 65543") || (has("OpenGL") && has("is not supported")) {
        return diag(
            "显卡驱动不支持这个版本要的 OpenGL",
            "更新显卡驱动（NVIDIA、AMD 或 Intel 官网下载）。双显卡的笔记本，到 Windows「设置 → 显示 → 图形」里把 Java 设成「高性能」。",
            None,
            "",
        );
    }
    if has("EXCEPTION_ACCESS_VIOLATION") && ["nvoglv64.dll", "atio6axx.dll", "ig9icd64.dll", "ig75icd64.dll", "igxelpicd64.dll"].iter().any(|d| text.contains(d)) {
        return diag("显卡驱动崩溃了", "崩溃发生在显卡驱动里。更新显卡驱动通常能解决；装了光影的话先关掉光影试试。", None, "");
    }
    if has("Unable to establish loopback connection") {
        return diag(
            "Java 建不了本机的内部连接",
            "通常是加速器或代理软件（尤其是 TUN 模式）接管了本机连接。启动器已经自动换过临时目录，还是不行的话，先关掉加速器再启动试试。",
            None,
            "",
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(mods: &[String]) -> Ctx<'_> {
        Ctx { memory_mb: 4096, total_mb: 16384, java_major: 17, java_custom: false, mods }
    }

    #[test]
    fn recognizes_common_crashes() {
        let none: Vec<String> = vec![];
        let d = analyze(&["java.lang.UnsupportedClassVersionError: x has been compiled by a more recent version of the Java Runtime (class file version 65.0)".into()], "", &ctx(&none)).unwrap();
        assert_eq!(d.fix, Some(Fix::JavaMin { major: 21 }));

        let d = analyze(&["Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space".into()], "", &ctx(&none)).unwrap();
        assert_eq!(d.fix, Some(Fix::Memory { mb: 6144 }));

        let d = analyze(&[], "Found duplicate mods: Mod ID: 'jei' from mod files: jei-1.20.1-forge-15.2.0.jar, jei-1.20.1-forge-15.20.0.jar", &ctx(&none)).unwrap();
        assert_eq!(d.fix, Some(Fix::DisableMods { files: vec!["jei-1.20.1-forge-15.2.0.jar".into()] }));

        let mods = vec!["sodium-fabric-0.5.11+mc1.20.1.jar".to_string(), "lithium.jar".to_string()];
        let d = analyze(&["org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered".into(), "Caused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin apply for mod sodium failed sodium.mixins.json:x".into()], "", &ctx(&mods)).unwrap();
        assert_eq!(d.fix, Some(Fix::DisableMods { files: vec!["sodium-fabric-0.5.11+mc1.20.1.jar".into()] }));

        let d = analyze(&["Missing or unsupported mandatory dependencies:".into(), "\tMod ID: 'geckolib', Requested by: 'mowziesmobs', Expected range: '[4.4,)', Actual version: '[MISSING]'".into()], "", &ctx(&none)).unwrap();
        assert!(d.detail.contains("「mowziesmobs」需要「geckolib」[4.4,)，但没装"));

        assert!(analyze(&["[12:00:00] [Render thread/INFO]: Stopping!".into()], "", &ctx(&none)).is_none());
    }
}
