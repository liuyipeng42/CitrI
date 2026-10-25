use crate::constants::{TOOLBAR_DEFAULT_HEIGHT, TOOLBAR_DEFAULT_WIDTH};
use tauri::{Manager, PhysicalPosition};

pub(super) fn position_toolbar(app: &tauri::AppHandle, points: (f64, f64, f64, f64)) {
    let Some(toolbar) = app.get_webview_window("toolbar") else {
        return;
    };
    let size = toolbar.outer_size().unwrap_or_else(|_| {
        tauri::PhysicalSize::new(TOOLBAR_DEFAULT_WIDTH, TOOLBAR_DEFAULT_HEIGHT)
    });
    let (start_x, start_y, end_x, end_y) = points;
    let vertical = end_y - start_y;
    let horizontal = end_x - start_x;
    let mut x = if vertical.abs() > 14.0 {
        if vertical > 0.0 {
            end_x - size.width as f64
        } else {
            end_x
        }
    } else if horizontal > 0.0 {
        end_x - size.width as f64
    } else {
        end_x
    };
    let mut y = if vertical < -14.0 {
        end_y - size.height as f64 - 12.0
    } else {
        end_y + 16.0
    };
    if let Ok(monitors) = toolbar.available_monitors() {
        let monitor = monitors
            .iter()
            .find(|monitor| {
                let position = monitor.position();
                let monitor_size = monitor.size();
                end_x >= position.x as f64
                    && end_x < (position.x + monitor_size.width as i32) as f64
                    && end_y >= position.y as f64
                    && end_y < (position.y + monitor_size.height as i32) as f64
            })
            .or_else(|| monitors.first());
        if let Some(monitor) = monitor {
            let position = monitor.position();
            let monitor_size = monitor.size();
            let min_x = position.x as f64;
            let min_y = position.y as f64;
            let max_x =
                (position.x + monitor_size.width as i32 - size.width as i32).max(position.x) as f64;
            let max_y = (position.y + monitor_size.height as i32 - size.height as i32)
                .max(position.y) as f64;
            x = x.clamp(min_x, max_x);
            y = y.clamp(min_y, max_y);
        }
    }
    let _ = toolbar.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

pub(super) fn hide_toolbar_when_outside(app: &tauri::AppHandle, x: f64, y: f64) {
    let ui_app = app.clone();
    let dispatcher = app.clone();
    let _ = dispatcher.run_on_main_thread(move || {
        let Some(toolbar) = ui_app.get_webview_window("toolbar") else {
            return;
        };
        if !toolbar.is_visible().unwrap_or(false) {
            return;
        }
        let inside = toolbar
            .outer_position()
            .ok()
            .zip(toolbar.outer_size().ok())
            .is_some_and(|(position, size)| {
                x >= position.x as f64
                    && x <= (position.x + size.width as i32) as f64
                    && y >= position.y as f64
                    && y <= (position.y + size.height as i32) as f64
            });
        if !inside {
            let _ = toolbar.hide();
        }
    });
}

pub fn hide_toolbar(app: &tauri::AppHandle) {
    let ui_app = app.clone();
    let dispatcher = app.clone();
    let _ = dispatcher.run_on_main_thread(move || {
        if let Some(toolbar) = ui_app.get_webview_window("toolbar") {
            let _ = toolbar.hide();
        }
    });
}
