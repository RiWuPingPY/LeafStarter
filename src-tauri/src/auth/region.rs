//! D6：离线账户按地区放开。中国大陆（国家代码 CN）直接允许；大陆以外要先在这台设备上登录过一次正版。
//!
//! 先问 Cloudflare（我们官网所在的 Cloudflare 会在 /cdn-cgi/trace 里告诉你它看到的国家），
//! 问的时候不走系统代理，免得开着加速器的大陆玩家被当成海外；问不到再看系统的国家或地区设置。

use std::time::Duration;

const TRACE: &str = "https://mcdownloader.pingyi.me/cdn-cgi/trace";

pub async fn in_mainland() -> bool {
    match ask_cloudflare().await {
        Some(loc) => loc == "CN",
        None => system_region_is_cn(),
    }
}

async fn ask_cloudflare() -> Option<String> {
    let http = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(6)).build().ok()?;
    let text = http.get(TRACE).send().await.ok()?.text().await.ok()?;
    parse_loc(&text)
}

fn parse_loc(trace: &str) -> Option<String> {
    trace.lines().find_map(|l| l.strip_prefix("loc=")).map(|s| s.trim().to_ascii_uppercase()).filter(|s| s.len() == 2)
}

/// Windows「国家或地区」设置；45 是中国
fn system_region_is_cn() -> bool {
    #[cfg(windows)]
    {
        extern "system" {
            fn GetUserGeoID(class: u32) -> i32;
        }
        // SAFETY: 只读系统设置，没有指针参数
        return unsafe { GetUserGeoID(16) } == 45;
    }
    #[allow(unreachable_code)]
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_trace() {
        assert_eq!(super::parse_loc("fl=1\nip=1.2.3.4\nloc=CN\ntls=TLSv1.3\n").as_deref(), Some("CN"));
        assert_eq!(super::parse_loc("loc=hk"), Some("HK".into()));
        assert_eq!(super::parse_loc("ip=1.2.3.4"), None);
    }
}
