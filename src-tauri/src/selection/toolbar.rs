use crate::{
    constants::{TOOLBAR_DEFAULT_HEIGHT, TOOLBAR_DEFAULT_WIDTH},
    desktop::windows::monitor_bounds,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, PhysicalPosition, WebviewWindow};

static TOOLBAR_VISIBLE: AtomicBool = AtomicBool::new(false);

pub(super) fn show_toolbar(toolbar: &WebviewWindow) {
    TOOLBAR_VISIBLE.store(true, Ordering::Release);
    let _ = toolbar.show();
    let _ = toolbar.set_always_on_top(true);
}

pub fn hide_toolbar_window(toolbar: &WebviewWindow) {
    TOOLBAR_VISIBLE.store(false, Ordering::Release);
    let _ = toolbar.hide();
}

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
    }
    .round() as i32;
    let mut y = if vertical < -14.0 {
        end_y - size.height as f64 - 12.0
    } else {
        end_y + 16.0
    }
    .round() as i32;
    if let Some((min_x, max_x, min_y, max_y)) = monitor_bounds(
        &toolbar,
        end_x.round() as i32,
        end_y.round() as i32,
        size,
        0,
    ) {
        x = x.clamp(min_x, max_x);
        y = y.clamp(min_y, max_y);
    }
    let _ = toolbar.set_position(PhysicalPosition::new(x, y));
}

pub(super) fn hide_toolbar_when_outside(app: &tauri::AppHandle, x: f64, y: f64) {
    if !TOOLBAR_VISIBLE.load(Ordering::Acquire) {
        return;
    }
    let ui_app = app.clone();
    let dispatcher = app.clone();
    let _ = dispatcher.run_on_main_thread(move || {
        let Some(toolbar) = ui_app.get_webview_window("toolbar") else {
            return;
        };
        if !toolbar.is_visible().unwrap_or(false) {
            TOOLBAR_VISIBLE.store(false, Ordering::Release);
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
            hide_toolbar_window(&toolbar);
        }
    });
}

pub fn hide_toolbar(app: &tauri::AppHandle) {
    if !TOOLBAR_VISIBLE.swap(false, Ordering::AcqRel) {
        return;
    }
    let ui_app = app.clone();
    let dispatcher = app.clone();
    let _ = dispatcher.run_on_main_thread(move || {
        if let Some(toolbar) = ui_app.get_webview_window("toolbar") {
            let _ = toolbar.hide();
        }
    });
}
