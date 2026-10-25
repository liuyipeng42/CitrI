use super::require_window;
use crate::{models::PlatformInfo, state::AppState};
use tauri::WebviewWindow;
use tauri_runtime::ResizeDirection;

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
