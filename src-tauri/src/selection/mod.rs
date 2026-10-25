mod capture;
mod filters;
mod input;
mod toolbar;
#[cfg(target_os = "windows")]
mod windows;

use capture::{CaptureGuard, capture_selection};
use input::InputActivity;
#[cfg(not(target_os = "windows"))]
use rdev::listen;
use rdev::{Button, Event, EventType, Key};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;
pub use toolbar::hide_toolbar;
use toolbar::hide_toolbar_when_outside;

enum ListenerEvent {
    Input(Event),
    #[cfg(target_os = "windows")]
    Reset,
}

fn listen_events(sender: mpsc::Sender<ListenerEvent>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        windows::listener::listen(sender)
    }
    #[cfg(not(target_os = "windows"))]
    {
        listen(move |event| {
            let _ = sender.send(ListenerEvent::Input(event));
        })
        .map_err(|error| format!("{error:?}"))
    }
}

#[derive(Default)]
struct Gesture {
    cursor: (f64, f64),
    pressed: Option<(f64, f64, Instant)>,
    last_release: Option<(f64, f64, Instant)>,
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
                match event {
                    ListenerEvent::Input(event) => {
                        handle_event(&worker_app, &gesture, &enabled, &generation, &input, event);
                    }
                    #[cfg(target_os = "windows")]
                    ListenerEvent::Reset => {
                        generation.fetch_add(1, Ordering::AcqRel);
                        input.note_pointer_activity();
                        *gesture.lock().unwrap_or_else(|error| error.into_inner()) =
                            Gesture::default();
                    }
                }
            }
        });
        thread::spawn(move || {
            // Keep the OS hook callback short: locks, UI dispatch and worker
            // creation must not delay the hook and cause Windows to remove it.
            let mut reported_error = false;
            let mut retry_delay = Duration::from_secs(1);
            loop {
                let result = listen_events(event_sender.clone());
                if let Err(error) = result
                    && !reported_error
                {
                    let message = format!("划词监听器启动失败：{error}");
                    eprintln!("[selection] {message}");
                    let event_app = app.clone();
                    let dispatcher = app.clone();
                    let _ = dispatcher.run_on_main_thread(move || {
                        let _ = event_app.emit_to("settings", "selection-error", message);
                    });
                    reported_error = true;
                }
                // The listener normally runs indefinitely, but may return if
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
mod tests;
