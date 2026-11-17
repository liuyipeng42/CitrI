mod assistant;
mod commands;
mod constants;
mod data;
mod desktop;
mod history;
mod http;
mod models;
mod selection;
mod settings;
mod state;
mod storage;

use settings::SettingsStore;
pub(crate) use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            desktop::ui::show_settings(app)
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
            data::start_auto_sync(app.handle().clone());
            desktop::windows::create_toolbar(app.handle())?;
            desktop::tray::create(app.handle())?;
            app.state::<AppState>()
                .selection
                .start(app.handle().clone());
            if should_show_settings {
                desktop::ui::show_settings(app.handle());
            }
            Ok(())
        })
        .on_window_event(desktop::events::handle)
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::history::list_history,
            commands::history::get_history_entry,
            commands::history::clear_history,
            data::commands::get_data_info,
            data::commands::save_sync_preferences,
            data::commands::set_sync_editing,
            data::commands::export_data,
            data::commands::import_data,
            data::commands::test_webdav,
            data::commands::sync_webdav,
            data::commands::open_data_directory,
            commands::settings::test_api,
            commands::window::platform_info,
            commands::toolbar::toolbar_state,
            commands::toolbar::toolbar_resize,
            commands::toolbar::perform_action,
            commands::actions::get_action_session,
            commands::actions::assistant_start,
            commands::actions::assistant_stop,
            commands::targets::clipboard_write,
            commands::window::window_control,
            commands::window::settings_window_control,
            commands::targets::open_target,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CitrI");
}
