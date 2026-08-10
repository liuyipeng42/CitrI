use crate::{models::Settings, state::AppState};
use tauri::{Emitter, Manager};

pub fn broadcast_settings(app: &tauri::AppHandle, settings: &Settings) {
    let public = crate::models::PublicSettings::from(settings);
    for (_, window) in app.webview_windows() {
        let _ = window.emit("settings-changed", &public);
    }
}

pub fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn set_enabled(app: &tauri::AppHandle, enabled: bool) {
    let state = app.state::<AppState>();
    let mut settings = state.settings.get();
    settings.enabled = enabled;
    if let Ok(settings) = state.settings.save(settings) {
        state.selection.set_enabled(enabled);
        if !enabled {
            crate::selection::hide_toolbar(app);
        }
        broadcast_settings(app, &settings);
    }
}
