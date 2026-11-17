use crate::{models::Settings, state::AppState};
use tauri::{Emitter, Manager};

pub fn apply_settings(app: &tauri::AppHandle, settings: &Settings) {
    let state = app.state::<AppState>();
    state.selection.set_enabled(settings.enabled);
    if !settings.enabled {
        crate::selection::hide_toolbar(app);
    }
    if let Some(toggle) = state.tray_toggle() {
        let _ = toggle.set_checked(settings.enabled);
    }
    let public = crate::models::PublicSettings::from(settings);
    let _ = app.emit("settings-changed", &public);
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
    let Ok(_data_guard) = state.lock_data() else {
        return;
    };
    let mut settings = state.settings.get();
    settings.enabled = enabled;
    if let Ok(settings) = state.settings.save(settings) {
        let _ = state.sync.note_settings(&settings);
        apply_settings(app, &settings);
    }
}
