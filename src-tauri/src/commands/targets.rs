use tauri::WebviewWindow;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
pub fn clipboard_write(
    window: WebviewWindow,
    app: tauri::AppHandle,
    text: String,
) -> Result<(), String> {
    if !window.label().starts_with("action-")
        && window.label() != "toolbar"
        && window.label() != "settings"
    {
        return Err("当前窗口无权写入剪贴板".into());
    }
    app.clipboard()
        .write_text(text)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_target(window: WebviewWindow, target: String) -> Result<(), String> {
    if !window.label().starts_with("action-") && window.label() != "settings" {
        return Err("当前窗口无权执行此操作".into());
    }
    let parsed = reqwest::Url::parse(&target).map_err(|_| "链接格式不正确".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("只允许打开 HTTP 或 HTTPS 链接".into());
    }
    open::that_detached(parsed.as_str()).map_err(|error| format!("无法打开链接：{error}"))
}

pub(super) fn is_uri_or_file_path(value: &str) -> bool {
    let text = value.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return false;
    }
    let safe_uri = reqwest::Url::parse(text)
        .ok()
        .is_some_and(|url| matches!(url.scheme(), "http" | "https" | "file"));
    safe_uri
        || (text.len() > 3
            && text.as_bytes()[1] == b':'
            && matches!(text.as_bytes()[2], b'/' | b'\\'))
        || (text.starts_with('/') && !text.starts_with("//"))
}
