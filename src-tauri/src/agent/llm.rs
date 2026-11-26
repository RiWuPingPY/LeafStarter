//! 模型接入（ROADMAP 5.13）：两种接口格式
//! - Anthropic Messages（Claude）：原始 HTTP（Rust 没有官方 SDK），`anthropic-version: 2026-10-21`。
//!   回复里的 content 原样放回历史（思考块要原样回传）；Claude Opus 5.5 思考不能关，用 `output_config.effort` 控制深浅；
//!   默认带上服务端兜底（`fallbacks: "default"`），安全分类器拒答时自动换模型。
//! - OpenAI 兼容（DeepSeek、通义千问、Kimi、智谱、本机 llama.cpp / LM Studio / vLLM，Ollama 也走它的 /v1）
//!
//! 对上层只暴露一种「一轮」：发历史 + 工具，拿回文字、工具调用、用量。历史按各自的格式存，不来回转换。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    /// 预设名：claude / deepseek / qwen / kimi / glm / custom / ollama / llama
    pub preset: String,
    pub name: String,
    /// anthropic / openai / ollama
    pub kind: String,
    pub base: String,
    pub model: String,
    /// vault 加密过的 Key（本机的源可以没有）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    /// 推理强度（Anthropic）：low / medium / high / xhigh / max
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 原样透传的额外参数（JSON 对象）
    #[serde(default)]
    pub extra: Option<Value>,
}

impl Source {
    pub fn key(&self) -> Result<String> {
        if self.key.is_empty() {
            return Ok(String::new());
        }
        String::from_utf8(crate::auth::vault::open(&self.key)?).context("保存的 Key 坏了，重新填一次")
    }
    fn anthropic(&self) -> bool {
        self.kind == "anthropic"
    }
}

/// 一个工具（给模型看的）
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

pub struct Turn {
    pub text: String,
    pub calls: Vec<ToolCall>,
    /// 原样放回历史的那条 assistant 消息
    pub message: Value,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// end_turn / tool_use / max_tokens / refusal / pause_turn（OpenAI 的 finish_reason 换成同样的词）
    pub stop: String,
}

pub fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION"))).connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(600)).build()?)
}

/// 系统提示和用户第一句话怎么放进历史
pub fn user_message(src: &Source, text: &str) -> Value {
    let _ = src;
    json!({ "role": "user", "content": text })
}

/// 工具结果放回历史：Anthropic 是一条 user 消息里放全部 tool_result；OpenAI 是每个结果一条 tool 消息
pub fn tool_results(src: &Source, results: &[(String, String, bool)]) -> Vec<Value> {
    if src.anthropic() {
        vec![json!({ "role": "user", "content": results.iter().map(|(id, text, err)| json!({ "type": "tool_result", "tool_use_id": id, "content": text, "is_error": err })).collect::<Vec<_>>() })]
    } else {
        results.iter().map(|(id, text, _)| json!({ "role": "tool", "tool_call_id": id, "content": text })).collect()
    }
}

/// 跑一轮
pub async fn turn(http: &reqwest::Client, src: &Source, system: &str, history: &[Value], tools: &[ToolSpec]) -> Result<Turn> {
    if src.anthropic() {
        anthropic_turn(http, src, system, history, tools).await
    } else {
        openai_turn(http, src, system, history, tools).await
    }
}

fn merge_extra(body: &mut Value, extra: &Option<Value>) {
    if let (Some(Value::Object(e)), Value::Object(b)) = (extra, body) {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
}

async fn anthropic_turn(http: &reqwest::Client, src: &Source, system: &str, history: &[Value], tools: &[ToolSpec]) -> Result<Turn> {
    let base = if src.base.trim().is_empty() { "https://api.anthropic.com" } else { src.base.trim().trim_end_matches('/') };
    let model = if src.model.is_empty() { "claude-opus-5-5" } else { &src.model };
    let mut body = json!({
        "model": model,
        "max_tokens": src.max_tokens.unwrap_or(16000),
        "system": system,
        "messages": history,
    });
    if !tools.is_empty() {
        body["tools"] = json!(tools.iter().map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.schema })).collect::<Vec<_>>());
    }
    let claude5 = model.starts_with("claude-");
    if claude5 && !src.effort.is_empty() {
        body["output_config"] = json!({ "effort": src.effort });
    }
    // 安全分类器拒答时让服务端自动换个模型接着答（新一代 Claude 模型都支持）
    let fallback = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-fable-5-2", "claude-fable-5-1", "claude-opus-5"].contains(&model);
    if fallback {
        body["fallbacks"] = json!("default");
    }
    merge_extra(&mut body, &src.extra);
    let mut req = http.post(format!("{base}/v1/messages")).header("anthropic-version", "2026-10-21").header("content-type", "application/json").json(&body);
    if fallback {
        req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
    }
    let key = src.key()?;
    if !key.is_empty() {
        req = req.header("x-api-key", key);
    }
    let r = req.send().await.context("连不上 Claude 的接口")?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("{}", api_error(status.as_u16(), &v));
    }
    let content = v["content"].as_array().cloned().unwrap_or_default();
    let text = content.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("");
    let calls = content
        .iter()
        .filter(|b| b["type"] == "tool_use")
        .map(|b| ToolCall { id: b["id"].as_str().unwrap_or("").into(), name: b["name"].as_str().unwrap_or("").into(), input: b["input"].clone() })
        .collect();
    Ok(Turn {
        text,
        calls,
        // 整个 content 原样放回去（思考块、兜底块都要原样回传）
        message: json!({ "role": "assistant", "content": content }),
        input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0) + v["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0),
        output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
        stop: v["stop_reason"].as_str().unwrap_or("end_turn").into(),
    })
}

async fn openai_turn(http: &reqwest::Client, src: &Source, system: &str, history: &[Value], tools: &[ToolSpec]) -> Result<Turn> {
    let mut base = src.base.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        bail!("这个 AI 源没填地址");
    }
    // Ollama 的 OpenAI 兼容接口在 /v1 下面
    if src.kind == "ollama" && !base.ends_with("/v1") {
        base.push_str("/v1");
    }
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(history.iter().cloned());
    let mut body = json!({
        "model": src.model,
        "messages": messages,
    });
    if !tools.is_empty() {
        body["tools"] = json!(tools.iter().map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.schema } })).collect::<Vec<_>>());
    }
    if let Some(m) = src.max_tokens {
        body["max_tokens"] = json!(m);
    }
    merge_extra(&mut body, &src.extra);
    let mut req = http.post(format!("{base}/chat/completions")).json(&body);
    let key = src.key()?;
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }
    let r = req.send().await.context("连不上 AI 接口")?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("{}", api_error(status.as_u16(), &v));
    }
    let choice = &v["choices"][0];
    let msg = choice["message"].clone();
    let calls = msg["tool_calls"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| {
                    let args = &c["function"]["arguments"];
                    // 参数一般是 JSON 字符串，有的本机模型直接给对象
                    let input = match args {
                        Value::String(s) => serde_json::from_str(s).unwrap_or(json!({})),
                        other => other.clone(),
                    };
                    ToolCall { id: c["id"].as_str().unwrap_or("call").into(), name: c["function"]["name"].as_str().unwrap_or("").into(), input }
                })
                .collect()
        })
        .unwrap_or_default();
    let mut message = json!({ "role": "assistant", "content": msg["content"].as_str().unwrap_or("") });
    if msg.get("tool_calls").is_some_and(|t| t.is_array()) {
        message["tool_calls"] = msg["tool_calls"].clone();
    }
    let stop = match choice["finish_reason"].as_str().unwrap_or("stop") {
        "tool_calls" => "tool_use",
        "length" => "max_tokens",
        "content_filter" => "refusal",
        _ => "end_turn",
    };
    Ok(Turn {
        text: msg["content"].as_str().unwrap_or("").to_string(),
        calls,
        message,
        input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        stop: stop.into(),
    })
}

fn api_error(status: u16, v: &Value) -> String {
    let msg = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).or_else(|| v["message"].as_str()).unwrap_or("");
    let why = match status {
        401 | 403 => "Key 不对或者没有权限",
        404 => "地址或者模型名不对",
        429 => "请求太频繁或者额度用完了",
        400 => "请求参数不被接受（这个模型可能不认某个参数）",
        500..=599 => "AI 服务那边出错了，等一会儿再试",
        _ => "请求失败",
    };
    if msg.is_empty() {
        format!("{why}（{status}）")
    } else {
        format!("{why}（{status}）：{}", msg.chars().take(300).collect::<String>())
    }
}

/// 不带工具问一次（翻译、写命令这类）：返回文字和用掉的 token
pub async fn complete(http: &reqwest::Client, src: &Source, system: &str, user: &str) -> Result<(String, u64)> {
    complete_with_images(http, src, system, user, &[]).await
}

/// 带图问一次（皮肤参考图）。images 是（媒体类型, base64），图放在文字前面
pub async fn complete_with_images(http: &reqwest::Client, src: &Source, system: &str, user: &str, images: &[(String, String)]) -> Result<(String, u64)> {
    let msg = if images.is_empty() {
        user_message(src, user)
    } else {
        let mut content: Vec<Value> = images
            .iter()
            .map(|(m, d)| {
                if src.anthropic() {
                    json!({ "type": "image", "source": { "type": "base64", "media_type": m, "data": d } })
                } else {
                    json!({ "type": "image_url", "image_url": { "url": format!("data:{m};base64,{d}") } })
                }
            })
            .collect();
        content.push(json!({ "type": "text", "text": user }));
        json!({ "role": "user", "content": content })
    };
    let t = turn(http, src, system, &[msg], &[]).await?;
    if t.stop == "refusal" {
        bail!("模型拒绝了这个请求");
    }
    Ok((t.text, t.input_tokens + t.output_tokens))
}

/// 测试连接：发一个最小请求（带一个工具，顺便看能不能调工具）
pub async fn test(src: &Source) -> Result<Value> {
    let http = client()?;
    let t0 = std::time::Instant::now();
    let tool = ToolSpec { name: "ping", description: "Return pong. Call this tool once.", schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }) };
    let h = vec![user_message(src, "Call the ping tool once, then say ok.")];
    let t = turn(&http, src, "You are a connectivity test.", &h, &[tool]).await?;
    Ok(json!({ "ms": t0.elapsed().as_millis() as u64, "tools": !t.calls.is_empty(), "text": t.text.chars().take(80).collect::<String>() }))
}
