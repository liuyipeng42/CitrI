mod protocol;

use crate::models::ApiProfile;
use futures_util::StreamExt;
pub use protocol::{CANCELLED, ChatCompletion, ChatDelta};
use protocol::{
    ChatResponse, append_delta, check_api_error, extract_delta, next_sse_block, parse_sse_block,
    require_response_text,
};
use reqwest::{Client, Url, header};
use serde_json::{Value, json};
use std::{sync::OnceLock, time::Duration};
use tokio_util::sync::CancellationToken;

/// Upper bound for a single, still incomplete SSE event.
const MAX_PENDING_EVENT_BYTES: usize = 2 * 1024 * 1024;

fn api_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    crate::http::shared_client(&CLIENT, || {
        Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .build()
    })
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

pub fn response_system_prompt(profile: &ApiProfile) -> String {
    const FORMAT: &str = "请使用 Markdown 格式输出回答，按内容需要使用标题、列表、表格、引用和代码块。代码块注明语言；不要将整个回答包在 Markdown 代码块中，不要输出 HTML。";
    let custom = profile.system_prompt.trim();
    if custom.is_empty() {
        FORMAT.into()
    } else {
        format!("{custom}\n\n{FORMAT}")
    }
}

fn request_body(profile: &ApiProfile, prompt: &str) -> Value {
    let mut body = profile.extra_body.clone();
    let messages = json!([
        { "role": "system", "content": response_system_prompt(profile) },
        { "role": "user", "content": prompt }
    ]);
    body.insert("model".into(), Value::String(profile.model.clone()));
    body.insert("messages".into(), messages);
    body.insert("temperature".into(), json!(profile.temperature));
    body.insert("max_tokens".into(), json!(profile.max_tokens));
    body.insert("stream".into(), Value::Bool(true));
    Value::Object(body)
}

pub async fn stream_chat_completion(
    profile: &ApiProfile,
    prompt: &str,
    cancellation: &CancellationToken,
    on_delta: impl FnMut(ChatDelta),
) -> ChatCompletion {
    let mut response = ChatResponse::default();
    let outcome = stream_into(profile, prompt, cancellation, &mut response, on_delta).await;
    ChatCompletion {
        content: response.content,
        error: outcome.err(),
    }
}

async fn stream_into(
    profile: &ApiProfile,
    prompt: &str,
    cancellation: &CancellationToken,
    result: &mut ChatResponse,
    mut on_delta: impl FnMut(ChatDelta),
) -> Result<(), String> {
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
        _ = cancellation.cancelled() => return Err(CANCELLED.into()),
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
            _ = cancellation.cancelled() => return Err(CANCELLED.into()),
            payload = response.json() => payload.map_err(|error| format!("API 响应格式错误：{error}"))?,
        };
        check_api_error(&payload)?;
        append_delta(result, extract_delta(&payload), &mut on_delta);
        return require_response_text(result);
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    loop {
        let next = tokio::select! {
            _ = cancellation.cancelled() => return Err(CANCELLED.into()),
            next = stream.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        buffer.extend_from_slice(&chunk.map_err(|error| format!("API 流读取失败：{error}"))?);
        while let Some(block) = next_sse_block(&mut buffer) {
            if cancellation.is_cancelled() {
                return Err(CANCELLED.into());
            }
            let block =
                String::from_utf8(block).map_err(|_| "API 流不是有效 UTF-8 文本".to_string())?;
            let (done, delta) = parse_sse_block(&block)?;
            if done {
                return require_response_text(result);
            }
            append_delta(result, delta, &mut on_delta);
        }
        // Only the unfinished remainder counts: one network chunk may carry
        // many complete events.
        if buffer.len() > MAX_PENDING_EVENT_BYTES {
            return Err("API 流中的单个事件过大".into());
        }
    }
    if !buffer.iter().all(u8::is_ascii_whitespace) {
        let block =
            String::from_utf8(buffer).map_err(|_| "API 流不是有效 UTF-8 文本".to_string())?;
        let (_, delta) = parse_sse_block(&block)?;
        append_delta(result, delta, &mut on_delta);
    }
    require_response_text(result)
}

#[cfg(test)]
mod tests;
