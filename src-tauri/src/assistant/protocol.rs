use serde_json::Value;

/// Error marker for requests stopped by the user, replaced or closed windows.
pub const CANCELLED: &str = "__CANCELLED__";

/// Streaming chunks use `delta`, non-streaming responses use `message`.
fn choice_field<'a>(choice: Option<&'a Value>, field: &str) -> Option<&'a Value> {
    let choice = choice?;
    ["delta", "message"].into_iter().find_map(|kind| {
        choice
            .get(kind)?
            .get(field)
            .filter(|value| !value.is_null())
    })
}

fn extract_text(choice: Option<&Value>) -> String {
    let Some(content) = choice_field(choice, "content") else {
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
                    part.as_str()
                        .or_else(|| part.get("text").and_then(Value::as_str))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn check_api_error(payload: &Value) -> Result<(), String> {
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
    pub(super) finish_reason: Option<String>,
}

#[derive(Default)]
pub(super) struct ChatResponse {
    pub(super) content: String,
    pub(super) has_reasoning: bool,
    pub(super) finish_reason: Option<String>,
}

pub struct ChatCompletion {
    pub content: String,
    pub error: Option<String>,
}

pub(super) fn extract_delta(payload: &Value) -> ChatDelta {
    let choice = payload.pointer("/choices/0");
    let reasoning = ["reasoning_content", "reasoning"]
        .into_iter()
        .find_map(|field| {
            choice_field(choice, field)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
        })
        .unwrap_or_default()
        .to_owned();
    ChatDelta {
        content: extract_text(choice),
        reasoning,
        finish_reason: choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

pub(super) fn require_response_text(response: &ChatResponse) -> Result<(), String> {
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

pub(super) fn parse_sse_block(block: &str) -> Result<(bool, ChatDelta), String> {
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

pub(super) fn append_delta(
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

/// Removes the first complete SSE event (terminated by `\n\n` or `\r\n\r\n`).
pub(super) fn next_sse_block(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    // Both separators end with `\n`; a single scan finds whichever comes first.
    let (index, separator) = buffer
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, byte)| **byte == b'\n')
        .find_map(|(end, _)| {
            if buffer[end - 1] == b'\n' {
                Some((end - 1, 2))
            } else if end >= 3 && &buffer[end - 3..end] == b"\r\n\r" {
                Some((end - 3, 4))
            } else {
                None
            }
        })?;
    let block = buffer[..index].to_vec();
    buffer.drain(..index + separator);
    Some(block)
}
