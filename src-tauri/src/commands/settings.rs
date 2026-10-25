use super::require_window;
use crate::{
    assistant::stream_chat_completion,
    desktop::ui,
    models::{ApiProfile, Settings},
    state::AppState,
};
use tauri::{Emitter, WebviewWindow};
use tokio_util::sync::CancellationToken;

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
    let _data_guard = state.lock_data()?;
    let settings = state.settings.save(settings)?;
    state.sync.note_settings(&settings)?;
    ui::apply_settings(&app, &settings);
    let history_revision = state.history.revision();
    state.history.set_limit(settings.history_limit)?;
    if state.history.revision() != history_revision {
        let _ = app.emit_to("settings", "history-changed", ());
    }
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
    let result = stream_chat_completion(&profile, "请只回复 OK", &cancellation, |_| {}).await;
    match result.error {
        Some(error) => Err(error),
        None => Ok(result.content.trim().into()),
    }
}
