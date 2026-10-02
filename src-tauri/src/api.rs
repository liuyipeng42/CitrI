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

#[derive(Default, Debug)]
pub struct ChatDelta {
    pub content: String,
    pub reasoning: String,
    finish_reason: Option<String>,
}

#[derive(Default)]
struct ChatResponse {
    content: String,
    has_reasoning: bool,
    finish_reason: Option<String>,
}

pub struct ChatCompletion {
    pub content: String,
    pub error: Option<String>,
}

fn extract_delta(payload: &Value) -> ChatDelta {
    let reasoning = ["delta", "message"]
        .into_iter()
        .find_map(|kind| {
            ["reasoning_content", "reasoning"]
                .into_iter()
                .find_map(|field| {
                    payload
                        .pointer(&format!("/choices/0/{kind}/{field}"))
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                })
        })
        .unwrap_or_default()
        .to_owned();
    ChatDelta {
        content: extract_text(payload),
        reasoning,
        finish_reason: payload
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn require_response_text(response: &ChatResponse) -> Result<(), String> {
    if response.finish_reason.as_deref() == Some("length") {
        return Err(
            "已达到输出 Token 上限，回答可能未完成；请提高设置中的最大输出 Token 后重试".into(),
        );
    }
    if response.content.trim().is_empty() && response.has_reasoning {
        return Err("模型仅返回了思考内容，未返回最终回答；思考内容已保留，可提高最大输出 Token 或检查服务端结束原因后重试".into());
    }
    if response.content.trim().is_empty() {
        Err("API 未返回文本内容，请检查模型和接口配置".into())
    } else {
        Ok(())
    }
}

fn parse_sse_block(block: &str) -> Result<(bool, ChatDelta), String> {
    let data = block
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let data = data.trim();
    if data.is_empty() {
        return Ok((false, ChatDelta::default()));
    }
    if data == "[DONE]" {
        return Ok((true, ChatDelta::default()));
    }
    let payload: Value =
        serde_json::from_str(data).map_err(|error| format!("API 流数据格式错误：{error}"))?;
    check_api_error(&payload)?;
    Ok((false, extract_delta(&payload)))
}

fn append_delta(
    result: &mut ChatResponse,
    mut delta: ChatDelta,
    on_delta: &mut impl FnMut(ChatDelta),
) {
    result.content.push_str(&delta.content);
    result.has_reasoning |= !delta.reasoning.trim().is_empty();
    if delta.finish_reason.is_some() {
        result.finish_reason = delta.finish_reason.take();
    }
    if !delta.content.is_empty() || !delta.reasoning.is_empty() {
        on_delta(delta);
    }
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
        append_delta(result, extract_delta(&payload), &mut on_delta);
        return require_response_text(result);
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
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
            if cancellation.is_cancelled() {
                return Err("__CANCELLED__".into());
            }
            let block =
                String::from_utf8(block).map_err(|_| "API 流不是有效 UTF-8 文本".to_string())?;
            let (done, delta) = parse_sse_block(&block)?;
            if done {
                return require_response_text(result);
            }
            append_delta(result, delta, &mut on_delta);
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
mod tests {
    use super::*;

    #[test]
    fn reasoning_chunks_are_emitted_before_the_final_answer() {
        let mut result = ChatResponse::default();
        let mut events = Vec::new();
        for payload in [
            json!({"choices":[{"delta":{"reasoning_content":"分析问题"}}]}),
            json!({"choices":[{"delta":{"reasoning":"，核对结果"}}]}),
            json!({"choices":[{"delta":{"content":"最终回答"}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
            json!({"choices":[],"usage":{"completion_tokens":100}}),
        ] {
            let (done, delta) = parse_sse_block(&format!("data: {payload}")).unwrap();
            assert!(!done);
            append_delta(&mut result, delta, &mut |delta| events.push(delta));
        }
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].reasoning, "分析问题");
        assert!(events[0].content.is_empty());
        assert!(result.has_reasoning);
        assert_eq!(result.finish_reason.as_deref(), Some("stop"));
        require_response_text(&result).unwrap();
        assert_eq!(result.content, "最终回答");
    }

    #[test]
    fn reasoning_only_and_token_limit_have_specific_errors() {
        let payload = json!({"choices":[{"message":{"content":null,"reasoning_content":"思考内容"},"finish_reason":"length"}]});
        let delta = extract_delta(&payload);
        assert_eq!(delta.reasoning, "思考内容");
        let mut response = ChatResponse::default();
        append_delta(&mut response, delta, &mut |_| {});
        assert!(
            require_response_text(&response)
                .unwrap_err()
                .contains("Token 上限")
        );
        let payload = json!({"choices":[{"message":{"content":null,"reasoning_content":"思考内容"},"finish_reason":"stop"}]});
        let mut response = ChatResponse::default();
        append_delta(&mut response, extract_delta(&payload), &mut |_| {});
        assert!(
            require_response_text(&response)
                .unwrap_err()
                .contains("仅返回了思考内容")
        );
    }

    #[test]
    fn fragmented_utf8_stream_preserves_reasoning() {
        let bytes = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"思考\"}}]}\r\n\r\ndata: [DONE]\n\n".as_bytes();
        let mut buffer = Vec::new();
        let mut events = Vec::new();
        for byte in bytes {
            buffer.push(*byte);
            while let Some(block) = next_sse_block(&mut buffer) {
                events.push(parse_sse_block(&String::from_utf8(block).unwrap()).unwrap());
            }
        }
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].1.reasoning, "思考");
        assert!(events[1].0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn reports_errors_inside_successful_api_responses() {
        let payload = json!({"error": {"message": "quota exceeded"}});
        assert!(
            check_api_error(&payload)
                .unwrap_err()
                .contains("quota exceeded")
        );
        assert!(parse_sse_block(&format!("data: {payload}")).is_err());
        assert!(require_response_text(&ChatResponse::default()).is_err());
        assert!(
            require_response_text(&ChatResponse {
                content: "OK".into(),
                ..Default::default()
            })
            .is_ok()
        );
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
        assert_eq!(delta.content, "Hi");
        assert!(parse_sse_block("data: [DONE]").unwrap().0);
        let mut profile = ApiProfile::default();
        profile
            .extra_body
            .insert("stream".into(), Value::Bool(false));
        assert_eq!(request_body(&profile, "hello")["stream"], true);
        profile.system_prompt = "  准确翻译  ".into();
        profile.extra_body.insert("messages".into(), json!([]));
        let body = request_body(&profile, "hello");
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.starts_with("准确翻译\n\n"));
        assert!(system.contains("请使用 Markdown 格式输出回答"));
        assert_eq!(system, response_system_prompt(&profile));
        assert_eq!(body["messages"][1]["content"], "hello");
        profile.system_prompt.clear();
        assert!(response_system_prompt(&profile).starts_with("请使用 Markdown"));
    }

    #[test]
    fn reuses_the_api_client() {
        assert!(std::ptr::eq(api_client().unwrap(), api_client().unwrap()));
    }

    #[tokio::test]
    async fn streamed_failures_and_cancellation_keep_partial_content_without_a_second_buffer() {
        let body=b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n\n".to_vec();
        let (url, server) = crate::data::tests::mock_server(vec![
            (
                "200 OK".into(),
                "Content-Type: text/event-stream\r\n".into(),
                body.clone(),
            ),
            (
                "200 OK".into(),
                "Content-Type: text/event-stream\r\n".into(),
                body,
            ),
        ]);
        let profile = ApiProfile {
            api_url: url,
            api_key: "test-key".into(),
            ..ApiProfile::default()
        };
        let response =
            stream_chat_completion(&profile, "hello", &CancellationToken::new(), |_| {}).await;
        assert_eq!(response.content, "partial");
        assert!(response.error.unwrap().contains("Token 上限"));
        let cancellation = CancellationToken::new();
        let response =
            stream_chat_completion(&profile, "hello", &cancellation, |_| cancellation.cancel())
                .await;
        assert_eq!(response.content, "partial");
        assert_eq!(response.error.as_deref(), Some("__CANCELLED__"));
        server.join().unwrap();
    }
}
