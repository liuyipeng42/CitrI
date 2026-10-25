use rdev::Key;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_os = "windows"))]
use std::{collections::HashSet, sync::Mutex};

#[derive(Default)]
pub(super) struct InputActivity {
    epoch: AtomicU64,
    #[cfg(not(target_os = "windows"))]
    pressed_keys: Mutex<HashSet<Key>>,
}

impl InputActivity {
    pub(super) fn snapshot(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub(super) fn note_key_press(&self, _key: Key) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        #[cfg(not(target_os = "windows"))]
        self.pressed_keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(_key);
    }

    pub(super) fn note_key_release(&self, _key: Key) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        #[cfg(not(target_os = "windows"))]
        self.pressed_keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&_key);
    }

    pub(super) fn note_pointer_activity(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn is_idle_since(&self, expected_epoch: u64) -> bool {
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

    pub(super) fn is_idle_since_with(
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
pub(super) fn windows_keyboard_idle() -> bool {
    // Locking the desktop (e.g. Win+L) can swallow key-up events. Query the
    // current state on the capture worker, never inside a low-level hook:
    // Windows updates asynchronous key state only after the hook returns.
    // Codes 1..=6 are mouse buttons/reserved, not keyboard keys.
    (8..=254).all(|key| unsafe { winapi::um::winuser::GetAsyncKeyState(key) >= 0 })
}
