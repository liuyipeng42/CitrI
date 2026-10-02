use crate::{
    AppState,
    constants::{TOOLBAR_DEFAULT_HEIGHT, TOOLBAR_DEFAULT_WIDTH},
    models::{FilterMode, SelectionPayload},
};
use rdev::{Button, Event, EventType, Key, listen};
#[cfg(not(target_os = "windows"))]
use std::collections::HashSet;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, PhysicalPosition};
#[cfg(target_os = "windows")]
#[path = "selection_windows.rs"]
mod native_selection;
#[cfg(target_os = "macos")]
use tauri_plugin_clipboard_manager::ClipboardExt;

const DEFAULT_WINDOWS_BLACKLIST: &[&str] = &[
    "explorer.exe",
    "snipaste.exe",
    "pixpin.exe",
    "sharex.exe",
    "excel.exe",
    "powerpnt.exe",
    "photoshop.exe",
    "illustrator.exe",
    "adobe premiere pro.exe",
    "afterfx.exe",
    "adobe audition.exe",
    "blender.exe",
    "3dsmax.exe",
    "maya.exe",
    "acad.exe",
    "sldworks.exe",
    "mstsc.exe",
];
const DEFAULT_MACOS_BLACKLIST: &[&str] = &["com.apple.finder", "finder"];
const SELECTION_SETTLE_DELAY: Duration = Duration::from_millis(160);

#[derive(Default)]
struct Gesture {
    cursor: (f64, f64),
    pressed: Option<(f64, f64, Instant)>,
    last_release: Option<(f64, f64, Instant)>,
}

#[derive(Default)]
struct InputActivity {
    epoch: AtomicU64,
    #[cfg(not(target_os = "windows"))]
    pressed_keys: Mutex<HashSet<Key>>,
}

impl InputActivity {
    fn snapshot(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    fn note_key_press(&self, _key: Key) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        #[cfg(not(target_os = "windows"))]
        self.pressed_keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(_key);
    }

    fn note_key_release(&self, _key: Key) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        #[cfg(not(target_os = "windows"))]
        self.pressed_keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&_key);
    }

    fn note_pointer_activity(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    fn is_idle_since(&self, expected_epoch: u64) -> bool {
        self.is_idle_since_with(expected_epoch, || {
            #[cfg(target_os = "windows")]
            {
                windows_keyboard_idle()
            }
            #[cfg(not(target_os = "windows"))]
            {
                self.pressed_keys
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .is_empty()
            }
        })
    }

    fn is_idle_since_with(
        &self,
        expected_epoch: u64,
        keyboard_idle: impl FnOnce() -> bool,
    ) -> bool {
        self.epoch.load(Ordering::Acquire) == expected_epoch
            && keyboard_idle()
            && self.epoch.load(Ordering::Acquire) == expected_epoch
    }
}

#[cfg(target_os = "windows")]
fn windows_keyboard_idle() -> bool {
    // Locking the desktop (e.g. Win+L) can swallow key-up events. Query the
    // current state on the capture worker, never inside a low-level hook:
    // Windows updates asynchronous key state only after the hook returns.
    // Codes 1..=6 are mouse buttons/reserved, not keyboard keys.
    (8..=254).all(|key| unsafe { winapi::um::winuser::GetAsyncKeyState(key) >= 0 })
}

struct CaptureGuard {
    enabled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    input: Arc<InputActivity>,
    capture_id: u64,
    expected_input_epoch: u64,
}

impl CaptureGuard {
    fn is_current(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == self.capture_id
            && self.input.is_idle_since(self.expected_input_epoch)
    }
}

pub struct SelectionController {
    enabled: Arc<AtomicBool>,
    started: AtomicBool,
    generation: Arc<AtomicU64>,
    input: Arc<InputActivity>,
}

impl SelectionController {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
            started: AtomicBool::new(false),
            generation: Arc::new(AtomicU64::new(0)),
            input: Arc::new(InputActivity::default()),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        if !enabled {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub fn start(&self, app: tauri::AppHandle) {
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let enabled = self.enabled.clone();
        let generation = self.generation.clone();
        let input = self.input.clone();
        let (event_sender, event_receiver) = mpsc::channel();
        let worker_app = app.clone();
        thread::spawn(move || {
            let gesture = Arc::new(Mutex::new(Gesture::default()));
            for event in event_receiver {
                handle_event(&worker_app, &gesture, &enabled, &generation, &input, event);
            }
        });
        thread::spawn(move || {
            // Keep the OS hook callback short: locks, UI dispatch and worker
            // creation must not delay the hook and cause Windows to remove it.
            let mut reported_error = false;
            let mut retry_delay = Duration::from_secs(1);
            loop {
                let sender = event_sender.clone();
                let result = listen(move |event| {
                    let _ = sender.send(event);
                });
                if let Err(error) = result {
                    if !reported_error {
                        let message = format!("划词监听器启动失败：{error:?}");
                        eprintln!("[selection] {message}");
                        let event_app = app.clone();
                        let dispatcher = app.clone();
                        let _ = dispatcher.run_on_main_thread(move || {
                            let _ = event_app.emit_to("settings", "selection-error", message);
                        });
                        reported_error = true;
                    }
                }
                // `rdev::listen` normally runs indefinitely, but may return if
                // the OS message loop ends or hook installation temporarily fails.
                thread::sleep(retry_delay);
                retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
            }
        });
    }
}

fn handle_event(
    app: &tauri::AppHandle,
    gesture: &Arc<Mutex<Gesture>>,
    enabled: &Arc<AtomicBool>,
    generation: &Arc<AtomicU64>,
    input: &Arc<InputActivity>,
    event: Event,
) {
    match event.event_type {
        EventType::MouseMove { x, y } => {
            if let Ok(mut current) = gesture.lock() {
                current.cursor = (x, y);
            }
        }
        EventType::ButtonPress(Button::Left) => {
            generation.fetch_add(1, Ordering::AcqRel);
            input.note_pointer_activity();
            let (x, y) = gesture
                .lock()
                .map(|mut current| {
                    current.pressed = Some((current.cursor.0, current.cursor.1, Instant::now()));
                    current.cursor
                })
                .unwrap_or_default();
            hide_toolbar_when_outside(app, x, y);
        }
        EventType::ButtonRelease(Button::Left) => {
            input.note_pointer_activity();
            if !enabled.load(Ordering::Acquire) {
                return;
            }
            let candidate = gesture.lock().ok().and_then(|mut current| {
                let (x, y) = current.cursor;
                let now = Instant::now();
                let pressed = current.pressed.take();
                let is_double_click = current.last_release.is_some_and(|(last_x, last_y, last)| {
                    now.duration_since(last) <= Duration::from_millis(450)
                        && (x - last_x).hypot(y - last_y) <= 5.0
                });
                current.last_release = Some((x, y, now));
                pressed.and_then(|(start_x, start_y, started)| {
                    let distance = (x - start_x).hypot(y - start_y);
                    let duration = now.duration_since(started);
                    (is_double_click || (distance >= 4.0 && duration <= Duration::from_secs(10)))
                        .then_some((start_x, start_y, x, y))
                })
            });
            if let Some(points) = candidate {
                let capture_id = generation.fetch_add(1, Ordering::AcqRel) + 1;
                let expected_input_epoch = input.snapshot();
                let app = app.clone();
                let guard = CaptureGuard {
                    enabled: enabled.clone(),
                    generation: generation.clone(),
                    input: input.clone(),
                    capture_id,
                    expected_input_epoch,
                };
                thread::spawn(move || capture_selection(app, guard, points));
            }
        }
        EventType::ButtonPress(_) | EventType::ButtonRelease(_) => {
            generation.fetch_add(1, Ordering::AcqRel);
            input.note_pointer_activity();
            hide_toolbar(app);
        }
        EventType::Wheel { .. } => {
            generation.fetch_add(1, Ordering::AcqRel);
            input.note_pointer_activity();
            hide_toolbar(app);
        }
        EventType::KeyPress(key) => {
            input.note_key_press(key);
            if !is_modifier(key) {
                hide_toolbar(app);
            }
        }
        EventType::KeyRelease(key) => {
            input.note_key_release(key);
        }
    }
}

fn capture_selection(app: tauri::AppHandle, guard: CaptureGuard, points: (f64, f64, f64, f64)) {
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
            let _ = toolbar.show();
            let _ = toolbar.set_always_on_top(true);
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
    let text = native_selection::read_selection(point).unwrap_or_default();
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

fn application_allowed(
    mode: &FilterMode,
    custom: &[String],
    app_name: Option<&str>,
    process_name: Option<&str>,
) -> bool {
    let candidates = [app_name, process_name]
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_lowercase())
        .collect::<Vec<_>>();
    if candidates
        .iter()
        .any(|candidate| candidate == "citri" || candidate == "citri.exe")
    {
        return false;
    }
    let matches = |entry: &str| {
        let entry = entry.trim().to_lowercase();
        candidates.iter().any(|candidate| {
            candidate == &entry
                || Path::new(candidate)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    == Some(entry.as_str())
        })
    };
    if *mode == FilterMode::Whitelist {
        return custom.iter().any(|entry| matches(entry));
    }
    let predefined = if cfg!(target_os = "windows") {
        DEFAULT_WINDOWS_BLACKLIST
    } else if cfg!(target_os = "macos") {
        DEFAULT_MACOS_BLACKLIST
    } else {
        &[]
    };
    !custom.iter().any(|entry| matches(entry))
        && !(*mode == FilterMode::Default && predefined.iter().any(|entry| matches(entry)))
}

fn position_toolbar(app: &tauri::AppHandle, points: (f64, f64, f64, f64)) {
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

fn hide_toolbar_when_outside(app: &tauri::AppHandle, x: f64, y: f64) {
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

fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::Alt
            | Key::AltGr
            | Key::MetaLeft
            | Key::MetaRight
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_applications() {
        assert!(!application_allowed(
            &FilterMode::Blacklist,
            &["code.exe".into()],
            Some("Code"),
            Some("code.exe")
        ));
        assert!(application_allowed(
            &FilterMode::Whitelist,
            &["code.exe".into()],
            Some("Code"),
            Some("code.exe")
        ));
        assert!(!application_allowed(
            &FilterMode::Whitelist,
            &["chrome.exe".into()],
            Some("Code"),
            Some("code.exe")
        ));
    }

    #[test]
    fn any_user_input_or_held_key_cancels_pending_capture() {
        let input = InputActivity::default();
        let epoch = input.snapshot();
        assert!(input.is_idle_since_with(epoch, || true));
        input.note_key_press(Key::KeyA);
        assert!(!input.is_idle_since_with(epoch, || true));
        input.note_key_release(Key::KeyA);
        assert!(!input.is_idle_since_with(epoch, || true));
        let epoch = input.snapshot();
        assert!(!input.is_idle_since_with(epoch, || false));
        assert!(input.is_idle_since_with(epoch, || true));
        input.note_pointer_activity();
        assert!(!input.is_idle_since_with(epoch, || true));
    }

    #[test]
    fn missing_lock_screen_key_up_does_not_block_future_captures() {
        let input = InputActivity::default();
        input.note_key_press(Key::MetaLeft);
        input.note_key_press(Key::KeyL);
        // The secure desktop consumes the releases; physical keys are now up.
        input.note_pointer_activity();
        let epoch = input.snapshot();
        assert!(input.is_idle_since_with(epoch, || true));
        // A key actually held after unlocking must still block capture.
        assert!(!input.is_idle_since_with(epoch, || false));
    }

    #[test]
    fn input_during_physical_state_query_cancels_capture() {
        let input = InputActivity::default();
        assert!(!input.is_idle_since_with(input.snapshot(), || {
            input.note_key_press(Key::ControlLeft);
            true
        }));
    }
}
