//! D6：离线账户按地区放开。IP 在中国大陆（国家代码 CN），或者系统的「国家或地区」设成中国，就直接允许；
//! 都不是的话，要先在这台设备上登录过一次正版。
//!
//! 系统地区先看（不用联网）；不是中国再问 Cloudflare（我们官网所在的 Cloudflare 会在 /cdn-cgi/trace 里告诉你它看到的国家），
//! 问的时候不走系统代理。11-25 e 总定：两个有一个是中国就行，因为开着 TUN 模式加速器时直连也会被接管，IP 会被看成海外。

use std::time::Duration;

const TRACE: &str = "https://mcdownloader.pingyi.me/cdn-cgi/trace";

pub async fn in_mainland() -> bool {
    system_region_is_cn() || ask_cloudflare().await.as_deref() == Some("CN")
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
