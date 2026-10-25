use super::require_window;
use crate::{
    models::{HistoryEntry, HistoryPage},
    state::AppState,
};
use tauri::{Emitter, Manager, WebviewWindow};

#[tauri::command]
pub async fn list_history(
    window: WebviewWindow,
    app: tauri::AppHandle,
    query: String,
    offset: usize,
    limit: usize,
) -> Result<HistoryPage, String> {
    require_window(&window, "settings")?;
    if query.len() > 8000 {
        return Err("搜索内容过长".into());
    }
    let ticket = app.state::<AppState>().history.next_query();
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>()
            .history
            .page_at(&query, offset, limit, ticket)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_history_entry(
    window: WebviewWindow,
    app: tauri::AppHandle,
    id: u64,
) -> Result<Option<HistoryEntry>, String> {
    require_window(&window, "settings")?;
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().history.entry(id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_history(window: WebviewWindow, app: tauri::AppHandle) -> Result<(), String> {
    require_window(&window, "settings")?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _data_guard = state.lock_data()?;
        state.history.clear()?;
        let _ = app.emit_to("settings", "history-changed", ());
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
