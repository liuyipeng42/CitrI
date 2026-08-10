mod api;
mod commands;
mod constants;
mod events;
mod models;
mod selection;
mod settings;
mod state;
mod tray;
mod ui;
mod windows;

use settings::SettingsStore;
pub(crate) use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            ui::show_settings(app)
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("settings.json");
            let settings = SettingsStore::load(path);
            let should_show_settings = settings.active_profile().api_key.is_empty();
            app.manage(AppState::new(settings));
            windows::create_toolbar(app.handle())?;
            tray::create(app.handle())?;
            app.state::<AppState>()
                .selection
                .start(app.handle().clone());
            if should_show_settings {
                ui::show_settings(app.handle());
            }
            Ok(())
        })
        .on_window_event(events::handle)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::test_api,
            commands::platform_info,
            commands::toolbar_state,
            commands::toolbar_resize,
            commands::perform_action,
            commands::get_action_session,
            commands::assistant_start,
            commands::assistant_stop,
            commands::clipboard_write,
            commands::window_control,
            commands::open_target,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CitrI");
}
