mod api;
mod commands;
mod constants;
mod data;
mod data_commands;
mod events;
mod history;
mod history_store;
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = app.path().data_dir()?.join("citri");
            let settings = SettingsStore::load(directory.join("settings.json"));
            let history = history::HistoryStore::load(
                directory.join("history.db"),
                settings.read(|settings| settings.history_limit),
            )?;
            let should_show_settings = settings.active_profile().api_key.is_empty();
            app.manage(AppState::new(settings, history));
            data_commands::start_auto_sync(app.handle().clone());
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
            commands::list_history,
            commands::get_history_entry,
            commands::clear_history,
            data_commands::get_data_info,
            data_commands::save_sync_preferences,
            data_commands::set_sync_editing,
            data_commands::export_data,
            data_commands::import_data,
            data_commands::test_webdav,
            data_commands::sync_webdav,
            data_commands::open_data_directory,
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
