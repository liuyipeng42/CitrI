use crate::models::ApiProfile;
use futures_util::StreamExt;
use reqwest::{Client, Url, header};
use serde_json::{Value, json};
use std::{sync::OnceLock, time::Duration};
use tokio_util::sync::CancellationToken;

static API_CLIENT: OnceLock<Client> = OnceLock::new();

fn api_client() -> Result<&'static Client, String> {
    if let Some(client) = API_CLIENT.get() {
        return Ok(client);
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|error| error.to_string())?;
    let _ = API_CLIENT.set(client);
    Ok(API_CLIENT
        .get()
        .expect("API client must be initialized after a successful build"))
}

pub fn normalize_chat_completions_url(value: &str) -> Result<String, String> {
    let raw = value.trim();
    if raw.is_empty() {
        return Err("请先填写 API URL".into());
    }
    let mut parsed = Url::parse(raw).map_err(|_| "API URL 格式不正确".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("API URL 必须使用 HTTP 或 HTTPS".into());
    }
    parsed.set_fragment(None);
    parsed.set_query(None);
    let clean_path = parsed.path().trim_end_matches('/');
    let path = if clean_path.to_lowercase().ends_with("/chat/completions") {
        clean_path.to_string()
    } else {
        format!("{clean_path}/chat/completions")
    };
    parsed.set_path(&path);
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

pub fn compose_prompt(template: &str, selected_text: &str) -> String {
    if template.is_empty() {
        return selected_text.to_string();
    }
    if template.contains("{{text}}") {
        template.replace("{{text}}", selected_text)
    } else {
        format!("{template}\n\n{selected_text}")
    }
}

fn extract_text(payload: &Value) -> String {
    let Some(content) = payload
        .pointer("/choices/0/delta/content")
        .or_else(|| payload.pointer("/choices/0/message/content"))
    else {
        return String::new();
    };
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    content
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    part.as_str().map(ToOwned::to_owned).or_else(|| {
                        part.get("text")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn check_api_error(payload: &Value) -> Result<(), String> {
    if let Some(error) = payload.get("error").filter(|error| !error.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
            .unwrap_or("服务返回了错误对象");
        return Err(format!(
            "API 错误：{}",
            message.chars().take(2000).collect::<String>()
        ));
    }
    Ok(())
}

fn require_response_text(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        Err("API 未返回文本内容，请检查模型和接口配置".into())
    } else {
        Ok(text)
    }
}

fn parse_sse_block(block: &str) -> Result<(bool, String), String> {
    let data = block
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let data = data.trim();
    if data.is_empty() {
        return Ok((false, String::new()));
    }
    if data == "[DONE]" {
        return Ok((true, String::new()));
    }
    let payload: Value =
        serde_json::from_str(data).map_err(|error| format!("API 流数据格式错误：{error}"))?;
    check_api_error(&payload)?;
    Ok((false, extract_text(&payload)))
}

fn next_sse_block(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    let (index, separator) = match (lf, crlf) {
        (Some(left), Some(right)) => {
            if left.0 <= right.0 {
                left
            } else {
                right
            }
        }
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return None,
    };
    let block = buffer[..index].to_vec();
    buffer.drain(..index + separator);
    Some(block)
}

fn request_body(profile: &ApiProfile, prompt: &str) -> Value {
    let mut body = profile.extra_body.clone();
    let mut messages = Vec::new();
    if !profile.system_prompt.trim().is_empty() {
        messages.push(json!({ "role": "system", "content": profile.system_prompt.trim() }));
    }
    messages.push(json!({ "role": "user", "content": prompt }));
    body.insert("model".into(), Value::String(profile.model.clone()));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("temperature".into(), json!(profile.temperature));
    body.insert("max_tokens".into(), json!(profile.max_tokens));
    body.insert("stream".into(), Value::Bool(true));
    Value::Object(body)
}

pub async fn stream_chat_completion(
    profile: &ApiProfile,
    prompt: &str,
    cancellation: &CancellationToken,
    mut on_delta: impl FnMut(String),
) -> Result<String, String> {
    if profile.api_key.trim().is_empty() {
        return Err("请先在设置中填写 API Key".into());
    }
    if profile.model.trim().is_empty() {
        return Err("请先在设置中填写模型名称".into());
    }
    let url = normalize_chat_completions_url(&profile.api_url)?;
    let request = api_client()?
        .post(url)
        .bearer_auth(profile.api_key.trim())
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "text/event-stream, application/json")
        .json(&request_body(profile, prompt));
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err("__CANCELLED__".into()),
        response = request.send() => response.map_err(|error| format!("API 请求失败：{error}"))?,
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if !status.is_success() {
        let reason = response
            .text()
            .await
            .unwrap_or_else(|_| status.canonical_reason().unwrap_or("请求失败").into());
        let reason = serde_json::from_str::<Value>(&reason)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/error/message")
                    .or_else(|| value.get("message"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .unwrap_or(reason);
        return Err(format!(
            "API {}: {}",
            status.as_u16(),
            reason.chars().take(2000).collect::<String>()
        ));
    }

    if !content_type.contains("text/event-stream") {
        let payload: Value = tokio::select! {
            _ = cancellation.cancelled() => return Err("__CANCELLED__".into()),
            payload = response.json() => payload.map_err(|error| format!("API 响应格式错误：{error}"))?,
        };
        check_api_error(&payload)?;
        let text = extract_text(&payload);
        if !text.is_empty() {
            on_delta(text.clone());
        }
        return require_response_text(text);
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut result = String::new();
    loop {
        let next = tokio::select! {
            _ = cancellation.cancelled() => return Err("__CANCELLED__".into()),
            next = stream.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        buffer.extend_from_slice(&chunk.map_err(|error| format!("API 流读取失败：{error}"))?);
        if buffer.len() > 2 * 1024 * 1024 {
            return Err("API 流中的单个事件过大".into());
        }
        while let Some(block) = next_sse_block(&mut buffer) {
            let block =
                String::from_utf8(block).map_err(|_| "API 流不是有效 UTF-8 文本".to_string())?;
            let (done, delta) = parse_sse_block(&block)?;
            if done {
                return require_response_text(result);
            }
            if !delta.is_empty() {
                result.push_str(&delta);
                on_delta(delta);
            }
        }
    }
    if !buffer.iter().all(u8::is_ascii_whitespace) {
        let block =
            String::from_utf8(buffer).map_err(|_| "API 流不是有效 UTF-8 文本".to_string())?;
        let (_, delta) = parse_sse_block(&block)?;
        if !delta.is_empty() {
            result.push_str(&delta);
            on_delta(delta);
        }
    }
    require_response_text(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_errors_inside_successful_api_responses() {
        let payload = json!({"error": {"message": "quota exceeded"}});
        assert!(
            check_api_error(&payload)
                .unwrap_err()
                .contains("quota exceeded")
        );
        assert!(parse_sse_block(&format!("data: {payload}")).is_err());
        assert!(require_response_text(String::new()).is_err());
        assert!(require_response_text(" \n".into()).is_err());
        assert_eq!(require_response_text("OK".into()).unwrap(), "OK");
    }

    #[test]
    fn normalizes_compatible_urls() {
        assert_eq!(
            normalize_chat_completions_url("https://example.com/v1/").unwrap(),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            normalize_chat_completions_url("https://example.com/openai/v1/chat/completions")
                .unwrap(),
            "https://example.com/openai/v1/chat/completions"
        );
        assert!(normalize_chat_completions_url("file:///tmp/api").is_err());
    }

    #[test]
    fn composes_prompts() {
        assert_eq!(compose_prompt("翻译：{{text}}", "hello"), "翻译：hello");
        assert_eq!(compose_prompt("解释", "hello"), "解释\n\nhello");
        assert_eq!(compose_prompt("", "hello"), "hello");
    }

    #[test]
    fn parses_sse_and_protects_core_fields() {
        let (done, delta) =
            parse_sse_block("data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}").unwrap();
        assert!(!done);
        assert_eq!(delta, "Hi");
        assert!(parse_sse_block("data: [DONE]").unwrap().0);
        let mut profile = ApiProfile::default();
        profile
            .extra_body
            .insert("stream".into(), Value::Bool(false));
        assert_eq!(request_body(&profile, "hello")["stream"], true);
    }

    #[test]
    fn reuses_the_api_client() {
        assert!(std::ptr::eq(api_client().unwrap(), api_client().unwrap()));
    }
}
