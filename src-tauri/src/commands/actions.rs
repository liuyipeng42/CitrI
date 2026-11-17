use super::require_window;
use crate::{
    assistant::{CANCELLED, compose_prompt, response_system_prompt, stream_chat_completion},
    history::now_millis,
    models::{ActionSession, HistoryEntry, HistoryStatus},
    state::{AppState, RequestHandle},
};
use tauri::{Emitter, WebviewWindow};
use tokio_util::sync::CancellationToken;

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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    require_window(&window, "action")?;
    let label = window.label().to_string();
    let session = state
        .with_session(&label, Clone::clone)
        .ok_or_else(|| "结果窗口会话已失效".to_string())?;
    let profile = state.settings.active_profile();
    let prompt = compose_prompt(&session.action.prompt, &session.selected_text);
    let history_ticket = state.history.begin();
    let created_at = now_millis();
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
            if !delta.reasoning.is_empty() {
                let _ = window.emit_to(label.as_str(), "assistant-reasoning", delta.reasoning);
            }
            if !delta.content.is_empty() {
                let _ = window.emit_to(label.as_str(), "assistant-delta", delta.content);
            }
        }
    })
    .await;
    // Stopped, replaced and closed windows still retain their partial response.
    // Only the active request may emit completion events to its result window.
    let cancelled = result.error.as_deref() == Some(CANCELLED);
    let error = (!cancelled).then_some(result.error).flatten();
    if let Some(ticket) = history_ticket {
        let status = if cancelled {
            HistoryStatus::Cancelled
        } else if error.is_some() {
            HistoryStatus::Error
        } else {
            HistoryStatus::Success
        };
        let system_prompt = response_system_prompt(&profile);
        let entry = HistoryEntry {
            sync_id: uuid::Uuid::new_v4().to_string(),
            revived_at: 0,
            id: ticket.id,
            created_at,
            action_name: session.action.name,
            model: profile.model,
            selected_text: session.selected_text,
            prompt,
            system_prompt,
            result: result.content,
            status,
            error: error.clone(),
        };
        let save_history = || {
            let _data_guard = state.lock_data()?;
            state.history.append(ticket, entry)
        };
        match save_history() {
            Ok(true) => {
                state.sync.request_sync();
                let _ = app.emit_to("settings", "history-changed", ());
            }
            Ok(false) => {}
            Err(error) => {
                let _ = app.emit_to("settings", "history-error", error);
            }
        }
    }
    if !state.finish_request(&label, request_id) {
        return Ok(());
    }
    if let Some(error) = error {
        let _ = window.emit_to(label.as_str(), "assistant-error", error);
    } else {
        let _ = window.emit_to(label.as_str(), "assistant-done", ());
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
        let _ = window.emit_to(window.label(), "assistant-done", ());
    }
    Ok(())
}
