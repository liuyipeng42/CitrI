use super::{filters::application_allowed, input::InputActivity, toolbar::position_toolbar};
use crate::{AppState, models::SelectionPayload};
#[cfg(target_os = "macos")]
use rdev::{EventType, Key};
#[cfg(target_os = "macos")]
use std::sync::Mutex;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use tauri::{Emitter, Manager};
#[cfg(target_os = "macos")]
use tauri_plugin_clipboard_manager::ClipboardExt;

const SELECTION_SETTLE_DELAY: Duration = Duration::from_millis(160);

pub(super) struct CaptureGuard {
    pub(super) enabled: Arc<AtomicBool>,
    pub(super) generation: Arc<AtomicU64>,
    pub(super) input: Arc<InputActivity>,
    pub(super) capture_id: u64,
    pub(super) expected_input_epoch: u64,
}

impl CaptureGuard {
    pub(super) fn is_current(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == self.capture_id
            && self.input.is_idle_since(self.expected_input_epoch)
    }
}

pub(super) fn capture_selection(
    app: tauri::AppHandle,
    guard: CaptureGuard,
    points: (f64, f64, f64, f64),
) {
    thread::sleep(SELECTION_SETTLE_DELAY);
    if !guard.is_current() {
        return;
    }
    let active_window = active_win_pos_rs::get_active_window().ok();
    let app_name = active_window.as_ref().map(|window| window.app_name.clone());
    let process_name = active_window
        .as_ref()
        .and_then(|window| window.process_path.file_name())
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned);
    let state = app.state::<AppState>();
    let allowed = state.settings.read(|settings| {
        application_allowed(
            &settings.filter_mode,
            &settings.filter_list,
            app_name.as_deref(),
            process_name.as_deref(),
        )
    });
    if !allowed {
        return;
    }
    if !guard.is_current() {
        return;
    }

    let text = selected_text(&app, &guard, (points.0, points.1));
    let text = text.trim().chars().take(200_000).collect::<String>();
    if text.is_empty() || guard.generation.load(Ordering::Acquire) != guard.capture_id {
        return;
    }
    let payload = SelectionPayload {
        text,
        x: points.2.round() as i32,
        y: points.3.round() as i32,
        app_name,
    };
    let ui_app = app.clone();
    let dispatcher = app.clone();
    let _ = dispatcher.run_on_main_thread(move || {
        if !guard.enabled.load(Ordering::Acquire)
            || guard.generation.load(Ordering::Acquire) != guard.capture_id
        {
            return;
        }
        ui_app
            .state::<AppState>()
            .set_latest_selection(payload.clone());
        position_toolbar(&ui_app, points);
        let _ = ui_app.emit_to("toolbar", "selection-selected", payload);
        if let Some(toolbar) = ui_app.get_webview_window("toolbar") {
            super::toolbar::show_toolbar(&toolbar);
        }
    });
}

#[cfg(target_os = "linux")]
fn selected_text(_app: &tauri::AppHandle, _guard: &CaptureGuard, _point: (f64, f64)) -> String {
    let Ok(clipboard) = x11_clipboard::Clipboard::new() else {
        return String::new();
    };
    clipboard
        .load(
            clipboard.getter.atoms.primary,
            clipboard.getter.atoms.utf8_string,
            clipboard.getter.atoms.property,
            Duration::from_millis(500),
        )
        .ok()
        .and_then(|contents| String::from_utf8(contents).ok())
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn selected_text(_app: &tauri::AppHandle, guard: &CaptureGuard, point: (f64, f64)) -> String {
    let text = super::windows::reader::read_selection(point).unwrap_or_default();
    if guard.is_current() {
        text
    } else {
        String::new()
    }
}

#[cfg(target_os = "macos")]
fn selected_text(app: &tauri::AppHandle, guard: &CaptureGuard, _point: (f64, f64)) -> String {
    // Captures must not back up or restore each other's temporary clipboard.
    static CAPTURE_LOCK: Mutex<()> = Mutex::new(());
    let Ok(_capture) = CAPTURE_LOCK.try_lock() else {
        return String::new();
    };
    enum ClipboardBackup {
        Text(String),
        Image(tauri::image::Image<'static>),
        Empty,
    }

    let clipboard = app.clipboard();
    let backup = if let Ok(text) = clipboard.read_text() {
        ClipboardBackup::Text(text)
    } else if let Ok(image) = clipboard.read_image() {
        ClipboardBackup::Image(image.to_owned())
    } else {
        ClipboardBackup::Empty
    };
    if !guard.is_current() {
        return String::new();
    }
    if clipboard.clear().is_err() {
        return String::new();
    }

    let modifier = Key::MetaLeft;
    let _ = rdev::simulate(&EventType::KeyPress(modifier));
    let _ = rdev::simulate(&EventType::KeyPress(Key::KeyC));
    let _ = rdev::simulate(&EventType::KeyRelease(Key::KeyC));
    let _ = rdev::simulate(&EventType::KeyRelease(modifier));

    let mut selected = String::new();
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(20));
        if let Ok(text) = clipboard.read_text()
            && !text.is_empty()
        {
            selected = text;
            break;
        }
    }
    match backup {
        ClipboardBackup::Text(text) => {
            let _ = clipboard.write_text(text);
        }
        ClipboardBackup::Image(image) => {
            let _ = clipboard.write_image(&image);
        }
        ClipboardBackup::Empty => {
            let _ = clipboard.clear();
        }
    }
    selected
}
