//! 云防火墙（ROADMAP 5.12 ⑤ 第 3 步）：轻量服务器默认只放行 22、80、443。
//! 用户填一个只有防火墙权限的子账号 AccessKey，启动器按服务器 IP 找到实例，加一条 TCP 规则。Key 只用这一次，不保存。
//! - 阿里云轻量（SWAS 2020-06-01）：RPC 签名（HMAC-SHA1），ListInstances、CreateFirewallRule
//! - 腾讯云轻量（Lighthouse 2020-03-24）：TC3-HMAC-SHA256，DescribeInstances、CreateFirewallRules

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const ALI_REGIONS: &[&str] = &["cn-hangzhou", "cn-shanghai", "cn-beijing", "cn-shenzhen", "cn-qingdao", "cn-zhangjiakou", "cn-huhehaote", "cn-chengdu", "cn-guangzhou", "cn-wulanchabu", "cn-hongkong", "ap-southeast-1", "ap-northeast-1", "us-west-1"];
const TC_REGIONS: &[&str] = &["ap-guangzhou", "ap-shanghai", "ap-beijing", "ap-chengdu", "ap-chongqing", "ap-nanjing", "ap-hongkong", "ap-singapore", "ap-tokyo", "na-siliconvalley"];

/// 秒级时间戳 → (年, 月, 日, 时, 分, 秒)（UTC）
fn utc(ts: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (ts / 86400) as i64;
    let secs = ts % 86400;
    // Howard Hinnant 的 civil_from_days
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d, (secs / 3600) as u32, (secs % 3600 / 60) as u32, (secs % 60) as u32)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?)
}

// ---------- 阿里云 ----------

fn ali_enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// RPC 签名：参数排好序 → GET&%2F&编码后的查询串 → HMAC-SHA1(密钥 + "&") → base64
pub fn ali_sign(params: &BTreeMap<String, String>, secret: &str) -> String {
    let q = params.iter().map(|(k, v)| format!("{}={}", ali_enc(k), ali_enc(v))).collect::<Vec<_>>().join("&");
    let to_sign = format!("GET&{}&{}", ali_enc("/"), ali_enc(&q));
    let mut mac = Hmac::<Sha1>::new_from_slice(format!("{secret}&").as_bytes()).expect("hmac");
    mac.update(to_sign.as_bytes());
    crate::base::util::base64(&mac.finalize().into_bytes())
}

async fn ali_call(region: &str, ak: &str, sk: &str, action: &str, extra: &[(&str, String)]) -> Result<Value> {
    let (y, mo, d, h, mi, s) = utc(now());
    let mut p: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in [
        ("Action", action.to_string()),
        ("Version", "2020-06-01".into()),
        ("Format", "JSON".into()),
        ("RegionId", region.into()),
        ("AccessKeyId", ak.into()),
        ("SignatureMethod", "HMAC-SHA1".into()),
        ("SignatureVersion", "1.0".into()),
        ("SignatureNonce", crate::sync::crypto::b64(&crate::sync::crypto::random::<12>())),
        ("Timestamp", format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")),
    ] {
        p.insert(k.into(), v);
    }
    for (k, v) in extra {
        p.insert((*k).into(), v.clone());
    }
    let sig = ali_sign(&p, sk);
    p.insert("Signature".into(), sig);
    let q = p.iter().map(|(k, v)| format!("{}={}", ali_enc(k), ali_enc(v))).collect::<Vec<_>>().join("&");
    let r = http()?.get(format!("https://swas.{region}.aliyuncs.com/?{q}")).send().await.context("连不上阿里云接口")?;
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if let Some(code) = v["Code"].as_str() {
        bail!("{code}：{}", v["Message"].as_str().unwrap_or(""));
    }
    Ok(v)
}

// ---------- 腾讯云 ----------

fn hmac256(key: &[u8], msg: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac");
    mac.update(msg.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// TC3-HMAC-SHA256 的 Authorization 头
pub fn tc3_auth(sid: &str, skey: &str, service: &str, host: &str, action: &str, payload: &str, ts: u64) -> String {
    let (y, m, d, ..) = utc(ts);
    let date = format!("{y:04}-{m:02}-{d:02}");
    let canonical = format!("POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{host}\nx-tc-action:{}\n\ncontent-type;host;x-tc-action\n{}", action.to_lowercase(), hex(&Sha256::digest(payload.as_bytes())));
    let scope = format!("{date}/{service}/tc3_request");
    let to_sign = format!("TC3-HMAC-SHA256\n{ts}\n{scope}\n{}", hex(&Sha256::digest(canonical.as_bytes())));
    let k = hmac256(format!("TC3{skey}").as_bytes(), &date);
    let k = hmac256(&k, service);
    let k = hmac256(&k, "tc3_request");
    format!("TC3-HMAC-SHA256 Credential={sid}/{scope}, SignedHeaders=content-type;host;x-tc-action, Signature={}", hex(&hmac256(&k, &to_sign)))
}

async fn tc_call(region: &str, sid: &str, skey: &str, action: &str, body: Value) -> Result<Value> {
    let host = "lighthouse.tencentcloudapi.com";
    let payload = body.to_string();
    let ts = now();
    let r = http()?
        .post(format!("https://{host}/"))
        .header("Authorization", tc3_auth(sid, skey, "lighthouse", host, action, &payload, ts))
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Host", host)
        .header("X-TC-Action", action)
        .header("X-TC-Timestamp", ts.to_string())
        .header("X-TC-Version", "2020-03-24")
        .header("X-TC-Region", region)
        .body(payload)
        .send()
        .await
        .context("连不上腾讯云接口")?;
    let v: Value = r.json().await.unwrap_or(Value::Null);
    let resp = v["Response"].clone();
    if let Some(e) = resp.get("Error") {
        bail!("{}：{}", e["Code"].as_str().unwrap_or(""), e["Message"].as_str().unwrap_or(""));
    }
    Ok(resp)
}

/// 按 IP 找到实例并放行端口。返回说明（哪个地域、哪台、加了什么）
pub async fn open_port(provider: &str, ak: &str, sk: &str, ip: &str, port: u16) -> Result<String> {
    let desc = "Minecraft (LeafStarter)";
    match provider {
        "aliyun" => {
            for region in ALI_REGIONS {
                let v = match ali_call(region, ak, sk, "ListInstances", &[("PublicIpAddresses", json!([ip]).to_string()), ("PageSize", "100".into())]).await {
                    Ok(v) => v,
                    Err(e) if e.to_string().contains("InvalidAccessKeyId") || e.to_string().contains("SignatureDoesNotMatch") => bail!("AccessKey 不对：{e:#}"),
                    Err(e) if e.to_string().contains("Forbidden") || e.to_string().contains("NoPermission") => bail!("这个 AccessKey 没有轻量服务器的权限：{e:#}"),
                    Err(_) => continue,
                };
                let Some(inst) = v["Instances"].as_array().and_then(|a| a.iter().find(|i| i["PublicIpAddress"].as_str() == Some(ip))) else { continue };
                let id = inst["InstanceId"].as_str().unwrap_or("").to_string();
                match ali_call(region, ak, sk, "CreateFirewallRule", &[("InstanceId", id.clone()), ("RuleProtocol", "TCP".into()), ("Port", port.to_string()), ("Remark", desc.into())]).await {
                    Ok(_) => return Ok(format!("阿里云 {region} 的 {id}：放行了 TCP {port}")),
                    Err(e) if e.to_string().to_lowercase().contains("exist") => return Ok(format!("阿里云 {region} 的 {id}：TCP {port} 本来就放行了")),
                    Err(e) => return Err(e).context("加防火墙规则失败"),
                }
            }
            bail!("在阿里云轻量服务器里没找到 IP 是 {ip} 的实例（这个 AccessKey 所属的账号下）")
        }
        "tencent" => {
            for region in TC_REGIONS {
                let v = match tc_call(region, ak, sk, "DescribeInstances", json!({ "Filters": [{ "Name": "public-ip-address", "Values": [ip] }] })).await {
                    Ok(v) => v,
                    Err(e) if e.to_string().contains("AuthFailure") => bail!("SecretId / SecretKey 不对或者没有权限：{e:#}"),
                    Err(_) => continue,
                };
                let Some(inst) = v["InstanceSet"].as_array().and_then(|a| a.first()) else { continue };
                let id = inst["InstanceId"].as_str().unwrap_or("").to_string();
                let rule = json!({ "InstanceId": id, "FirewallRules": [{ "Protocol": "TCP", "Port": port.to_string(), "CidrBlock": "0.0.0.0/0", "Action": "ACCEPT", "FirewallRuleDescription": desc }] });
                match tc_call(region, ak, sk, "CreateFirewallRules", rule).await {
                    Ok(_) => return Ok(format!("腾讯云 {region} 的 {id}：放行了 TCP {port}")),
                    Err(e) if e.to_string().contains("Exist") || e.to_string().contains("Duplicate") => return Ok(format!("腾讯云 {region} 的 {id}：TCP {port} 本来就放行了")),
                    Err(e) => return Err(e).context("加防火墙规则失败"),
                }
            }
            bail!("在腾讯云轻量服务器里没找到 IP 是 {ip} 的实例")
        }
        _ => bail!("不认识的云服务商"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates() {
        assert_eq!(utc(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(utc(1456231584), (2016, 2, 23, 12, 46, 24));
        assert_eq!(utc(1709251199), (2024, 2, 29, 23, 59, 59));
    }

    /// 阿里云文档里的签名示例（ECS DescribeRegions，密钥 testsecret）
    #[test]
    fn aliyun_doc_example() {
        let mut p = BTreeMap::new();
        for (k, v) in [
            ("AccessKeyId", "testid"),
            ("Action", "DescribeRegions"),
            ("Format", "XML"),
            ("SignatureMethod", "HMAC-SHA1"),
            ("SignatureNonce", "3ee8c1b8-83d3-44af-a94f-4e0ad82fd6cf"),
            ("SignatureVersion", "1.0"),
            ("Timestamp", "2016-02-23T12:46:24Z"),
            ("Version", "2014-05-26"),
        ] {
            p.insert(k.to_string(), v.to_string());
        }
        assert_eq!(ali_sign(&p, "testsecret"), "OLeaidS1JvxuMvnyHOwuJ+uX5qY=");
    }

    #[test]
    fn tc3_shape() {
        let a = tc3_auth("AKID", "KEY", "lighthouse", "lighthouse.tencentcloudapi.com", "DescribeInstances", "{}", 1551113065);
        assert!(a.starts_with("TC3-HMAC-SHA256 Credential=AKID/2019-02-25/lighthouse/tc3_request, SignedHeaders=content-type;host;x-tc-action, Signature="));
        assert_eq!(a.rsplit('=').next().unwrap().len(), 64);
    }
}
