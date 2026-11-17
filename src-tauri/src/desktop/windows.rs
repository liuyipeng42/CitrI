use crate::{
    AppState,
    constants::{
        ACTION_WINDOW_DEFAULT_HEIGHT, ACTION_WINDOW_DEFAULT_WIDTH, ACTION_WINDOW_MIN_HEIGHT,
        ACTION_WINDOW_MIN_WIDTH, TOOLBAR_DEFAULT_HEIGHT, TOOLBAR_DEFAULT_WIDTH,
    },
    models::{ActionItem, ActionWindowSession},
};
use tauri::{Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub fn create_toolbar(app: &tauri::AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window("toolbar") {
        return Ok(window);
    }
    WebviewWindowBuilder::new(app, "toolbar", WebviewUrl::App("toolbar.html".into()))
        .title("CitrI")
        .inner_size(TOOLBAR_DEFAULT_WIDTH as f64, TOOLBAR_DEFAULT_HEIGHT as f64)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .shadow(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()
}

pub fn create_action_window(
    app: &tauri::AppHandle,
    label: &str,
    action: ActionItem,
    selected_text: String,
) -> Result<WebviewWindow, String> {
    let state = app.state::<AppState>();
    let (auto_pin, follow_toolbar, width, height) = state.settings.read(|settings| {
        let (width, height) = if settings.remember_action_window_size {
            (settings.action_window_width, settings.action_window_height)
        } else {
            (ACTION_WINDOW_DEFAULT_WIDTH, ACTION_WINDOW_DEFAULT_HEIGHT)
        };
        (settings.auto_pin, settings.follow_toolbar, width, height)
    });
    state.insert_session(
        label.to_string(),
        ActionWindowSession {
            action,
            selected_text,
            pinned: auto_pin,
            frontend_ready: false,
            auto_close_armed: false,
            last_window_size: None,
            native_interaction_until: None,
        },
    );
    let result = WebviewWindowBuilder::new(app, label, WebviewUrl::App("action.html".into()))
        .title("CitrI")
        .inner_size(width as f64, height as f64)
        .min_inner_size(
            ACTION_WINDOW_MIN_WIDTH as f64,
            ACTION_WINDOW_MIN_HEIGHT as f64,
        )
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(true)
        .skip_taskbar(true)
        .always_on_top(auto_pin)
        .visible(false)
        .prevent_overflow_with_margin(tauri::LogicalSize::new(6.0, 6.0))
        .build()
        .map_err(|error| error.to_string());
    let window = match result {
        Ok(window) => window,
        Err(error) => {
            state.remove_session(label);
            return Err(format!("无法创建结果窗口：{error}"));
        }
    };
    if follow_toolbar {
        position_below_toolbar(app, &window);
    } else {
        let _ = window.center();
    }
    // Hide the non-focusable toolbar before transferring focus. Scheduling this
    // after set_focus can produce a late blur event and immediately auto-close
    // the newly created action window on Windows.
    if let Some(toolbar) = app.get_webview_window("toolbar") {
        crate::selection::hide_toolbar_window(&toolbar);
    }
    let _ = window.show();
    let _ = window.set_focus();
    Ok(window)
}

pub fn monitor_bounds(
    window: &WebviewWindow,
    anchor_x: i32,
    anchor_y: i32,
    size: tauri::PhysicalSize<u32>,
    margin: i32,
) -> Option<(i32, i32, i32, i32)> {
    let monitors = window.available_monitors().ok()?;
    let monitor = monitors
        .iter()
        .find(|monitor| {
            let position = monitor.position();
            let monitor_size = monitor.size();
            anchor_x >= position.x
                && anchor_x < position.x + monitor_size.width as i32
                && anchor_y >= position.y
                && anchor_y < position.y + monitor_size.height as i32
        })
        .or_else(|| monitors.first())?;
    let position = monitor.position();
    let monitor_size = monitor.size();
    let min_x = position.x + margin;
    let min_y = position.y + margin;
    let max_x = (position.x + monitor_size.width as i32 - size.width as i32 - margin).max(min_x);
    let max_y = (position.y + monitor_size.height as i32 - size.height as i32 - margin).max(min_y);
    Some((min_x, max_x, min_y, max_y))
}

fn position_below_toolbar(app: &tauri::AppHandle, action: &WebviewWindow) {
    let Some(toolbar) = app.get_webview_window("toolbar") else {
        let _ = action.center();
        return;
    };
    let (Ok(toolbar_position), Ok(toolbar_size), Ok(action_size)) = (
        toolbar.outer_position(),
        toolbar.outer_size(),
        action.outer_size(),
    ) else {
        let _ = action.center();
        return;
    };
    let mut x = toolbar_position.x + (toolbar_size.width as i32 - action_size.width as i32) / 2;
    let mut y = toolbar_position.y + toolbar_size.height as i32 + 8;
    if let Some((min_x, max_x, min_y, max_y)) = monitor_bounds(
        action,
        toolbar_position.x,
        toolbar_position.y,
        action_size,
        6,
    ) {
        if y > max_y {
            y = toolbar_position.y - action_size.height as i32 - 8;
        }
        for (other_label, other) in app.webview_windows() {
            if other_label.starts_with("action-")
                && other_label != action.label()
                && other.is_visible().unwrap_or(false)
                && let Ok(pos) = other.outer_position()
                && (pos.x - x).abs() < 24
                && (pos.y - y).abs() < 24
            {
                x += 28;
                y += 28;
            }
        }
        x = x.clamp(min_x, max_x);
        y = y.clamp(min_y, max_y);
    }
    let _ = action.set_position(PhysicalPosition::new(x, y));
}
