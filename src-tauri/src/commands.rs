use crate::{
    api::{compose_prompt, stream_chat_completion},
    models::{ActionSession, ApiProfile, PlatformInfo, Settings, ToolbarState},
    selection::hide_toolbar,
    state::{AppState, RequestHandle},
    ui, windows,
};
use tauri::{Emitter, WebviewWindow};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_runtime::ResizeDirection;
use tokio_util::sync::CancellationToken;

fn require_window(window: &WebviewWindow, expected: &str) -> Result<(), String> {
    let valid = if expected == "action" {
        window.label().starts_with("action-")
    } else {
        window.label() == expected
    };
    valid
        .then_some(())
        .ok_or_else(|| "当前窗口无权执行此操作".into())
}

#[tauri::command]
pub fn get_settings(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<Settings, String> {
    require_window(&window, "settings")?;
    Ok(state.settings.get())
}

#[tauri::command]
pub async fn save_settings(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    require_window(&window, "settings")?;
    let settings = state.settings.save(settings)?;
    state.selection.set_enabled(settings.enabled);
    if !settings.enabled {
        hide_toolbar(&app);
    }
    if let Some(item) = state.tray_toggle() {
        let _ = item.set_checked(settings.enabled);
    }
    ui::broadcast_settings(&app, &settings);
    Ok(settings)
}

#[tauri::command]
pub async fn test_api(window: WebviewWindow, profile: ApiProfile) -> Result<String, String> {
    require_window(&window, "settings")?;
    let profile = Settings {
        active_api_profile_id: profile.id.clone(),
        api_profiles: vec![profile],
        ..Settings::default()
    }
    .sanitized()
    .active_profile();
    let cancellation = CancellationToken::new();
    let result = stream_chat_completion(&profile, "请只回复 OK", &cancellation, |_| {}).await?;
    Ok(result.trim().into())
}

#[tauri::command]
pub fn platform_info() -> PlatformInfo {
    let display_protocol = if cfg!(target_os = "linux") {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "wayland"
        } else if std::env::var_os("DISPLAY").is_some() {
            "x11"
        } else {
            "unknown"
        }
    } else {
        "unknown"
    };
    PlatformInfo {
        platform: std::env::consts::OS.into(),
        display_protocol: display_protocol.into(),
    }
}

#[tauri::command]
pub fn toolbar_state(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<ToolbarState, String> {
    require_window(&window, "toolbar")?;
    Ok(ToolbarState {
        settings: state.settings.public(),
        selection: state.latest_selection(),
    })
}

#[tauri::command]
pub fn toolbar_resize(window: WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    require_window(&window, "toolbar")?;
    window
        .set_size(tauri::LogicalSize::new(
            width.clamp(48.0, 1000.0),
            height.clamp(36.0, 100.0),
        ))
        .map_err(|error| error.to_string())
}

#[tauri::command]
// 必须保持异步：Windows 上 Wry 动态创建 WebView 时会等待主事件循环处理创建消息。
pub async fn perform_action(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    action_id: String,
    selected_text: String,
) -> Result<bool, String> {
    require_window(&window, "toolbar")?;
    let selected_text = selected_text
        .trim()
        .chars()
        .take(200_000)
        .collect::<String>();
    if selected_text.is_empty() {
        return Ok(false);
    }
    let Some(action) = state.settings.read(|settings| {
        settings
            .actions
            .iter()
            .find(|action| action.id == action_id && action.enabled)
            .cloned()
    }) else {
        return Ok(false);
    };
    if action.id == "copy" {
        app.clipboard()
            .write_text(selected_text)
            .map_err(|error| error.to_string())?;
        return Ok(true);
    }
    if action.id == "search" {
        let search_url = state.settings.read(|settings| settings.search_url.clone());
        let target = if is_uri_or_file_path(&selected_text) {
            selected_text.trim().to_string()
        } else {
            search_url.replace(
                "{{queryString}}",
                &urlencoding::encode(selected_text.trim()),
            )
        };
        open::that_detached(&target).map_err(|error| format!("无法打开目标：{error}"))?;
        hide_toolbar(&app);
        return Ok(true);
    }
    let label = format!("action-{}", state.next_action_id());
    windows::create_action_window(&app, &label, action, selected_text)?;
    Ok(true)
}

#[tauri::command]
pub fn get_action_session(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<ActionSession, String> {
    require_window(&window, "action")?;
    let session = state
        .with_session(window.label(), Clone::clone)
        .ok_or_else(|| "结果窗口会话已失效".to_string())?;
    Ok(ActionSession {
        action: session.action,
        selected_text: session.selected_text,
        settings: state.settings.public(),
    })
}

#[tauri::command]
pub async fn assistant_start(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    require_window(&window, "action")?;
    let label = window.label().to_string();
    let session = state
        .with_session(&label, Clone::clone)
        .ok_or_else(|| "结果窗口会话已失效".to_string())?;
    let profile = state.settings.active_profile();
    let prompt = compose_prompt(&session.action.prompt, &session.selected_text);
    let request_id = state.next_request_id();
    let cancellation = CancellationToken::new();
    if let Some(previous) = state.replace_request(
        label.clone(),
        RequestHandle {
            id: request_id,
            cancellation: cancellation.clone(),
        },
    ) {
        previous.cancellation.cancel();
    }
    let result = stream_chat_completion(&profile, &prompt, &cancellation, |delta| {
        if state.request_is_current(&label, request_id) {
            let _ = window.emit("assistant-delta", delta);
        }
    })
    .await;
    if !state.finish_request(&label, request_id) {
        return Ok(());
    }
    if matches!(&result, Err(error) if error == "__CANCELLED__") {
        let _ = window.emit("assistant-done", ());
    } else if let Err(error) = result {
        let _ = window.emit("assistant-error", error);
    } else {
        let _ = window.emit("assistant-done", ());
    }
    Ok(())
}

#[tauri::command]
pub fn assistant_stop(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    require_window(&window, "action")?;
    if let Some(request) = state.remove_request(window.label()) {
        request.cancellation.cancel();
        let _ = window.emit("assistant-done", ());
    }
    Ok(())
}

#[tauri::command]
pub fn clipboard_write(
    window: WebviewWindow,
    app: tauri::AppHandle,
    text: String,
) -> Result<(), String> {
    if !window.label().starts_with("action-") && window.label() != "toolbar" {
        return Err("当前窗口无权写入剪贴板".into());
    }
    app.clipboard()
        .write_text(text)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn window_control(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
    operation: String,
    value: Option<bool>,
    direction: Option<String>,
) -> Result<bool, String> {
    require_window(&window, "action")?;
    match operation.as_str() {
        "close" => window.close().map_err(|error| error.to_string())?,
        "minimize" => window.minimize().map_err(|error| error.to_string())?,
        "drag" => {
            state.with_session_mut(window.label(), |session| {
                session.begin_native_interaction();
            });
            window.start_dragging().map_err(|error| error.to_string())?;
        }
        "resize" => {
            let direction = parse_resize_direction(direction.as_deref())?;
            state.with_session_mut(window.label(), |session| {
                session.begin_native_interaction();
            });
            window
                .as_ref()
                .window()
                .start_resize_dragging(direction)
                .map_err(|error| error.to_string())?;
        }
        "pin" => {
            let pinned = value.unwrap_or(false);
            state.with_session_mut(window.label(), |session| session.pinned = pinned);
            window
                .set_always_on_top(pinned)
                .map_err(|error| error.to_string())?;
            return Ok(pinned);
        }
        "ready" => {
            state.with_session_mut(window.label(), |session| {
                session.mark_frontend_ready(window.is_focused().unwrap_or(false));
            });
            return Ok(true);
        }
        _ => return Err("不支持的窗口操作".into()),
    }
    Ok(false)
}

fn parse_resize_direction(direction: Option<&str>) -> Result<ResizeDirection, String> {
    match direction {
        Some("north") => Ok(ResizeDirection::North),
        Some("north-east") => Ok(ResizeDirection::NorthEast),
        Some("east") => Ok(ResizeDirection::East),
        Some("south-east") => Ok(ResizeDirection::SouthEast),
        Some("south") => Ok(ResizeDirection::South),
        Some("south-west") => Ok(ResizeDirection::SouthWest),
        Some("west") => Ok(ResizeDirection::West),
        Some("north-west") => Ok(ResizeDirection::NorthWest),
        _ => Err("无效的窗口缩放方向".into()),
    }
}

#[tauri::command]
pub fn open_target(window: WebviewWindow, target: String) -> Result<(), String> {
    require_window(&window, "action")?;
    let parsed = reqwest::Url::parse(&target).map_err(|_| "链接格式不正确".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("只允许打开 HTTP 或 HTTPS 链接".into());
    }
    open::that_detached(parsed.as_str()).map_err(|error| format!("无法打开链接：{error}"))
}

fn is_uri_or_file_path(value: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_urls_and_file_paths() {
        assert!(is_uri_or_file_path("https://example.com"));
        assert!(is_uri_or_file_path("C:\\Temp\\file.txt"));
        assert!(is_uri_or_file_path("/tmp/file.txt"));
        assert!(!is_uri_or_file_path("javascript://alert(1)"));
        assert!(!is_uri_or_file_path("search these words"));
    }
}
