//! Own the Windows hooks so silent removal can be recovered without restarting
//! the application. Hook callbacks only copy input into the worker's queue;
//! keyboard layout/name resolution and UI Automation never run on this thread.
use crate::selection::{ListenerEvent, input::windows_keyboard_idle};
use rdev::{Button, Event, EventType, Key};
use std::{
    cell::RefCell,
    io,
    mem::{size_of, zeroed},
    ptr::null_mut,
    sync::mpsc::Sender,
    time::{Duration, Instant, SystemTime},
};
use winapi::{
    shared::{
        minwindef::{LPARAM, LRESULT, WPARAM},
        windef::HHOOK,
    },
    um::winuser::*,
};

const CHECK_INTERVAL_MS: u32 = 250;
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const RESUME_GAP: Duration = Duration::from_secs(5);
const QUIET_INTERVAL: Duration = Duration::from_secs(1);

struct HookContext {
    sender: Sender<ListenerEvent>,
    events: u64,
    last_event: Instant,
}

thread_local! {
    static CONTEXT: RefCell<Option<HookContext>> = const { RefCell::new(None) };
}

fn send(event_type: EventType) {
    CONTEXT.with(|context| {
        if let Some(context) = context.borrow_mut().as_mut() {
            context.events = context.events.wrapping_add(1);
            context.last_event = Instant::now();
            let _ = context.sender.send(ListenerEvent::Input(Event {
                event_type,
                time: SystemTime::now(),
                name: None,
            }));
        }
    });
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        let mouse = unsafe { &*(data as *const MSLLHOOKSTRUCT) };
        // Button messages carry their own coordinates. Do not depend on having
        // received an earlier move (especially immediately after recovery).
        send(EventType::MouseMove {
            x: mouse.pt.x as f64,
            y: mouse.pt.y as f64,
        });
        let event = match message as u32 {
            WM_LBUTTONDOWN => Some(EventType::ButtonPress(Button::Left)),
            WM_LBUTTONUP => Some(EventType::ButtonRelease(Button::Left)),
            WM_RBUTTONDOWN => Some(EventType::ButtonPress(Button::Right)),
            WM_RBUTTONUP => Some(EventType::ButtonRelease(Button::Right)),
            WM_MBUTTONDOWN => Some(EventType::ButtonPress(Button::Middle)),
            WM_MBUTTONUP => Some(EventType::ButtonRelease(Button::Middle)),
            WM_XBUTTONDOWN => Some(EventType::ButtonPress(Button::Unknown(
                (mouse.mouseData >> 16) as u8,
            ))),
            WM_XBUTTONUP => Some(EventType::ButtonRelease(Button::Unknown(
                (mouse.mouseData >> 16) as u8,
            ))),
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                let delta = ((mouse.mouseData >> 16) as i16 / WHEEL_DELTA) as i64;
                Some(if message as u32 == WM_MOUSEWHEEL {
                    EventType::Wheel {
                        delta_x: 0,
                        delta_y: delta,
                    }
                } else {
                    EventType::Wheel {
                        delta_x: delta,
                        delta_y: 0,
                    }
                })
            }
            _ => None,
        };
        if let Some(event) = event {
            send(event);
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}

fn key_from_vk(vk: u32) -> Key {
    match vk as i32 {
        VK_LSHIFT | VK_SHIFT => Key::ShiftLeft,
        VK_RSHIFT => Key::ShiftRight,
        VK_LCONTROL | VK_CONTROL => Key::ControlLeft,
        VK_RCONTROL => Key::ControlRight,
        VK_LMENU | VK_MENU => Key::Alt,
        VK_RMENU => Key::AltGr,
        VK_LWIN => Key::MetaLeft,
        VK_RWIN => Key::MetaRight,
        _ => Key::Unknown(vk),
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        let keyboard = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        let key = key_from_vk(keyboard.vkCode);
        match message as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => send(EventType::KeyPress(key)),
            WM_KEYUP | WM_SYSKEYUP => send(EventType::KeyRelease(key)),
            _ => {}
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}

struct Hooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

impl Hooks {
    fn install() -> io::Result<Self> {
        // RAII also releases the keyboard hook if mouse hook installation fails.
        let mut hooks = Self {
            keyboard: null_mut(),
            mouse: null_mut(),
        };
        hooks.keyboard =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), null_mut(), 0) };
        if hooks.keyboard.is_null() {
            return Err(io::Error::last_os_error());
        }
        hooks.mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0) };
        if hooks.mouse.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(hooks)
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        unsafe {
            if !self.keyboard.is_null() {
                UnhookWindowsHookEx(self.keyboard);
            }
            if !self.mouse.is_null() {
                UnhookWindowsHookEx(self.mouse);
            }
        }
    }
}

struct Timer(usize);
impl Drop for Timer {
    fn drop(&mut self) {
        unsafe {
            KillTimer(null_mut(), self.0);
        }
    }
}

fn system_last_input() -> io::Result<u32> {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.dwTime)
}

struct Watchdog {
    checked: Instant,
    refreshed: Instant,
    events: u64,
    system_input: u32,
    pending: bool,
}

impl Watchdog {
    fn new(now: Instant, system_input: u32) -> Self {
        Self {
            checked: now,
            refreshed: now,
            events: 0,
            system_input,
            pending: false,
        }
    }

    fn needs_refresh(
        &mut self,
        now: Instant,
        events: u64,
        system_input: u32,
        quiet: bool,
        idle: bool,
    ) -> bool {
        self.pending |= now.duration_since(self.checked) >= RESUME_GAP
            || (system_input != self.system_input && events == self.events)
            || (quiet && now.duration_since(self.refreshed) >= REFRESH_INTERVAL);
        self.checked = now;
        self.events = events;
        self.system_input = system_input;
        self.pending && idle
    }

    fn refreshed(&mut self, now: Instant) {
        self.refreshed = now;
        self.pending = false;
    }
}

pub(in crate::selection) fn listen(sender: Sender<ListenerEvent>) -> Result<(), String> {
    CONTEXT.with(|context| {
        *context.borrow_mut() = Some(HookContext {
            sender: sender.clone(),
            events: 0,
            last_event: Instant::now(),
        })
    });
    let result = listen_inner(&sender);
    CONTEXT.with(|context| *context.borrow_mut() = None);
    result.map_err(|error| error.to_string())
}

fn listen_inner(sender: &Sender<ListenerEvent>) -> io::Result<()> {
    let mut hooks = Hooks::install()?;
    let _ = sender.send(ListenerEvent::Reset);
    let timer = Timer(unsafe { SetTimer(null_mut(), 0, CHECK_INTERVAL_MS, None) });
    if timer.0 == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut watchdog = Watchdog::new(Instant::now(), system_last_input()?);
    let mut message = unsafe { zeroed::<MSG>() };
    loop {
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        if result == 0 {
            return Ok(());
        }
        if message.message == WM_TIMER && message.wParam == timer.0 {
            let now = Instant::now();
            let (events, quiet) = CONTEXT.with(|context| {
                let context = context.borrow();
                let context = context.as_ref().unwrap();
                (
                    context.events,
                    now.duration_since(context.last_event) >= QUIET_INTERVAL,
                )
            });
            // Never replace hooks in the middle of a drag or a held key.
            let idle =
                (1..=6).all(|key| unsafe { GetAsyncKeyState(key) >= 0 }) && windows_keyboard_idle();
            if watchdog.needs_refresh(now, events, system_last_input()?, quiet, idle) {
                // Remove old handles first, including a surviving half of a
                // silently removed pair. Never leave duplicate hooks installed.
                drop(hooks);
                hooks = Hooks::install()?;
                let _ = sender.send(ListenerEvent::Reset);
                watchdog.refreshed(now);
            }
        } else {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_removal_is_detected_even_while_message_loop_is_alive() {
        let now = Instant::now();
        let mut watchdog = Watchdog::new(now, 100);
        assert!(!watchdog.needs_refresh(now + Duration::from_millis(250), 4, 200, false, true));
        // Windows sees new input, but neither hook delivered it.
        assert!(watchdog.needs_refresh(now + Duration::from_millis(500), 4, 300, false, true));
    }

    #[test]
    fn recovery_waits_until_drag_or_held_key_finishes() {
        let now = Instant::now();
        let mut watchdog = Watchdog::new(now, 100);
        assert!(!watchdog.needs_refresh(now + Duration::from_millis(250), 0, 200, false, false));
        assert!(watchdog.needs_refresh(now + Duration::from_millis(500), 0, 200, false, true));
        watchdog.refreshed(now + Duration::from_millis(500));
        assert!(!watchdog.needs_refresh(now + Duration::from_millis(750), 1, 300, false, true));
    }

    #[test]
    fn resume_refreshes_without_waiting_for_new_input() {
        let now = Instant::now();
        let mut watchdog = Watchdog::new(now, 100);
        assert!(watchdog.needs_refresh(now + Duration::from_secs(10), 0, 100, true, true));
    }
}
