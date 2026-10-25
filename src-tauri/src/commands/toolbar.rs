use super::{require_window, targets::is_uri_or_file_path};
use crate::{desktop::windows, models::ToolbarState, selection::hide_toolbar, state::AppState};
use tauri::WebviewWindow;
use tauri_plugin_clipboard_manager::ClipboardExt;

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
) -> Result<bool, String> {
    require_window(&window, "toolbar")?;
    // Capture once from the authoritative store; toolbar initialization and
    // selection events can arrive out of order in the WebView.
    let Some(selection) = state.latest_selection() else {
        return Ok(false);
    };
    let selected_text = selection
        .text
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
    // Selecting text leaves the clipboard untouched. Commit it only when the
    // user explicitly invokes an action from the toolbar.
    app.clipboard()
        .write_text(selected_text.clone())
        .map_err(|error| error.to_string())?;
    if action.id == "copy" {
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
