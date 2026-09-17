use crate::state::AppState;
use std::{sync::mpsc, thread, time::Duration};
use tauri::Manager;

fn cursor_is_near_window(window: &tauri::Window) -> bool {
    let (Ok(cursor), Ok(position), Ok(size)) = (
        window.cursor_position(),
        window.outer_position(),
        window.outer_size(),
    ) else {
        return false;
    };
    const MARGIN: f64 = 10.0;
    let left = position.x as f64 - MARGIN;
    let top = position.y as f64 - MARGIN;
    let right = position.x as f64 + size.width as f64 + MARGIN;
    let bottom = position.y as f64 + size.height as f64 + MARGIN;
    cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom
}

fn schedule_action_auto_close(app: tauri::AppHandle, label: String) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(250));
            let dispatcher = app.clone();
            let check_app = app.clone();
            let check_label = label.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            if dispatcher
                .run_on_main_thread(move || {
                    let Some(webview_window) = check_app.get_webview_window(&check_label) else {
                        return;
                    };
                    let window = webview_window.as_ref().window();
                    let state = check_app.state::<AppState>();
                    if !state.settings.read(|settings| settings.auto_close) {
                        return;
                    }
                    let focused = window.is_focused().unwrap_or(true);
                    if focused || window.is_minimized().unwrap_or(true) {
                        return;
                    }
                    let cursor_near_window = cursor_is_near_window(&window);
                    let should_close = state
                        .with_session(&check_label, |session| {
                            session.should_close_after_blur(focused, cursor_near_window)
                        })
                        .unwrap_or(false);
                    if should_close {
                        let _ = window.close();
                        return;
                    }
                    // Cursor proximity and native drag/resize protection are temporary.
                    // Continue checking instead of losing the original blur forever.
                    let retry = state
                        .with_session(&check_label, |session| {
                            session.auto_close_armed && !session.pinned
                        })
                        .unwrap_or(false);
                    let _ = sender.send(retry);
                })
                .is_err()
                || !receiver.recv().unwrap_or(false)
            {
                break;
            }
        }
    });
}

pub fn handle(window: &tauri::Window, event: &tauri::WindowEvent) {
    let label = window.label().to_string();
    let state = window.state::<AppState>();
    match event {
        tauri::WindowEvent::CloseRequested { api, .. }
            if label == "settings" && !state.is_quitting() =>
        {
            api.prevent_close();
            let _ = window.hide();
        }
        tauri::WindowEvent::Focused(focused) if label.starts_with("action-") => {
            let should_schedule_close = state
                .with_session_mut(&label, |session| session.on_focus_changed(*focused))
                .unwrap_or(false);
            if !focused && should_schedule_close {
                schedule_action_auto_close(window.app_handle().clone(), label);
            }
        }
        tauri::WindowEvent::Resized(size) if label.starts_with("action-") => {
            note_size(window, &state, &label, size.width, size.height, None);
        }
        tauri::WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
            ..
        } if label.starts_with("action-") => {
            note_size(
                window,
                &state,
                &label,
                new_inner_size.width,
                new_inner_size.height,
                Some(*scale_factor),
            );
        }
        tauri::WindowEvent::Moved(_) if label.starts_with("action-") => {
            state.with_session_mut(&label, |session| session.note_native_window_change());
        }
        tauri::WindowEvent::Destroyed if label.starts_with("action-") => {
            if let Some((width, height)) = state
                .remove_session(&label)
                .and_then(|session| session.last_window_size)
            {
                let _ = state.settings.save_action_window_size(width, height);
            }
            if let Some(request) = state.remove_request(&label) {
                request.cancellation.cancel();
            }
        }
        _ => {}
    }
}

fn note_size(
    window: &tauri::Window,
    state: &AppState,
    label: &str,
    physical_width: u32,
    physical_height: u32,
    scale_factor: Option<f64>,
) {
    let scale_factor = scale_factor
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0))
        .max(0.1);
    let width = (physical_width as f64 / scale_factor).round() as u32;
    let height = (physical_height as f64 / scale_factor).round() as u32;
    state.with_session_mut(label, |session| session.note_window_size(width, height));
}
