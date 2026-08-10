use crate::{state::AppState, ui};
use tauri::{
    Manager,
    menu::{CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    let enabled = app
        .state::<AppState>()
        .settings
        .read(|settings| settings.enabled);
    let toggle = CheckMenuItemBuilder::with_id("toggle", "启用 CitrI")
        .checked(enabled)
        .build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", "设置…").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &separator_one,
            &settings_item,
            &separator_two,
            &quit,
        ],
    )?;
    app.state::<AppState>().set_tray_toggle(toggle);

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("CitrI")
        .show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => {
                let enabled = app
                    .state::<AppState>()
                    .tray_toggle()
                    .as_ref()
                    .and_then(|item| item.is_checked().ok())
                    .unwrap_or(true);
                ui::set_enabled(app, enabled);
            }
            "settings" => ui::show_settings(app),
            "quit" => {
                app.state::<AppState>().mark_quitting();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                ui::show_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
