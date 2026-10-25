use super::protocol::*;
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
        stream_chat_completion(&profile, "hello", &cancellation, |_| cancellation.cancel()).await;
    assert_eq!(response.content, "partial");
    assert_eq!(response.error.as_deref(), Some("__CANCELLED__"));
    server.join().unwrap();
}
