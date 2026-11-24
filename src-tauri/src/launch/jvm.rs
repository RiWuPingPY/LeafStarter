//! 给版本挑 Java、定内存。

use serde_json::Value;

use crate::game::java::JavaInfo;

/// 版本要哪个 Java：(大版本号, 官方运行时组件名)。老的 JSON 没写就当 Java 8
pub fn need_of(json: &Value) -> (u32, Option<String>) {
    let jv = json.get("javaVersion");
    let major = jv.and_then(|j| j.get("majorVersion")).and_then(|m| m.as_u64()).unwrap_or(8) as u32;
    let component = jv.and_then(|j| j.get("component")).and_then(|c| c.as_str()).map(String::from);
    (major, component)
}

/// 挑 Java：要 8 的必须正好是 8（老版本和老 Forge 在 9 以上起不来）；
/// 要 17 / 21 的优先同版本，没有就用比它新的里最旧的一个
pub fn choose(list: &[JavaInfo], need: u32) -> Option<&JavaInfo> {
    if need <= 8 {
        return list.iter().find(|j| j.major == 8);
    }
    list.iter().find(|j| j.major == need).or_else(|| list.iter().filter(|j| j.major > need).min_by_key(|j| j.major))
}

/// 本机内存（MB）；读不到就当 8 GB
pub fn total_memory_mb() -> u64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatusEx {
            length: u32,
            memory_load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page_file: u64,
            avail_page_file: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended_virtual: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
        }
        let mut m = MemoryStatusEx { length: std::mem::size_of::<MemoryStatusEx>() as u32, memory_load: 0, total_phys: 0, avail_phys: 0, total_page_file: 0, avail_page_file: 0, total_virtual: 0, avail_virtual: 0, avail_extended_virtual: 0 };
        // SAFETY: 结构体按 Win32 的 MEMORYSTATUSEX 布局声明，length 已填好
        if unsafe { GlobalMemoryStatusEx(&mut m) } != 0 && m.total_phys > 0 {
            return m.total_phys / 1024 / 1024;
        }
    }
    8192
}

/// 自动分配的内存：本机的四分之一，最少 2 GB、最多 6 GB，按 512 MB 取整
pub fn auto_memory_mb(total_mb: u64) -> u32 {
    let quarter = (total_mb / 4).clamp(2048, 6144);
    (quarter / 512 * 512) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(major: u32) -> JavaInfo {
        JavaInfo { path: format!("java{}", major), major, version: major.to_string(), source: String::new() }
    }

    #[test]
    fn picks_java() {
        let list = vec![j(25), j(21), j(17), j(8)];
        assert_eq!(choose(&list, 8).unwrap().major, 8);
        assert_eq!(choose(&list, 17).unwrap().major, 17);
        assert_eq!(choose(&[j(25), j(21)], 17).unwrap().major, 21);
        assert!(choose(&[j(17)], 8).is_none());
        assert!(choose(&[j(17)], 21).is_none());
    }

    #[test]
    fn memory() {
        assert_eq!(auto_memory_mb(4096), 2048);
        assert_eq!(auto_memory_mb(16384), 4096);
        assert_eq!(auto_memory_mb(65536), 6144);
        assert_eq!(auto_memory_mb(13000), 3072);
    }
}
