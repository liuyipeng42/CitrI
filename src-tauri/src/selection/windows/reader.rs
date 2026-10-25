use std::{
    sync::{Mutex, TryLockError},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::POINT,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::{
            Accessibility::{
                CUIAutomation8, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern,
                UIA_TextPatternId,
            },
            WindowsAndMessaging::GetForegroundWindow,
        },
    },
    core::Result,
};

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

// Capture workers own their COM objects. A slow provider must not accumulate
// concurrent UIA calls, block the input hook, or cause synthetic key presses.
pub(in crate::selection) fn read_selection(point: (f64, f64)) -> Result<String> {
    static READER: Mutex<()> = Mutex::new(());
    let _reader = match READER.try_lock() {
        Ok(reader) => reader,
        Err(TryLockError::Poisoned(error)) => error.into_inner(),
        Err(TryLockError::WouldBlock) => return Ok(String::new()),
    };
    let started = Instant::now();
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let _apartment = ComApartment;
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return Ok(String::new());
        }
        // CUIAutomation8 supports provider timeouts. The older client uses a
        // 20-second transaction timeout, holding READER across subsequent gestures.
        let automation: IUIAutomation2 =
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
        automation.SetConnectionTimeout(500)?;
        automation.SetTransactionTimeout(1_000)?;
        let root = automation.ElementFromHandle(foreground)?;
        let walker = automation.ControlViewWalker()?;
        let pointed = automation.ElementFromPoint(POINT {
            x: point.0.round() as i32,
            y: point.1.round() as i32,
        });
        let focused = automation.GetFocusedElement();
        for candidate in [pointed, focused] {
            let Ok(mut element) = candidate else { continue };
            for _ in 0..6 {
                if GetForegroundWindow() != foreground
                    || started.elapsed() >= Duration::from_secs(2)
                {
                    return Ok(String::new());
                }
                // Never read password controls, including through their parent.
                if element.CurrentIsPassword()?.as_bool() {
                    break;
                }
                if let Ok(text) = element_selection(&element, started)
                    && !text.trim().is_empty()
                {
                    return Ok(if GetForegroundWindow() == foreground {
                        text
                    } else {
                        String::new()
                    });
                }
                // Do not walk past the foreground window into the desktop.
                if automation.CompareElements(&element, &root)?.as_bool() {
                    break;
                }
                let Ok(parent) = walker.GetParentElement(&element) else {
                    break;
                };
                element = parent;
            }
        }
        Ok(String::new())
    }
}

unsafe fn element_selection(element: &IUIAutomationElement, started: Instant) -> Result<String> {
    unsafe {
        let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId)?;
        let ranges = pattern.GetSelection()?;
        let mut text = String::new();
        let mut remaining = 200_000;
        for index in 0..ranges.Length()?.min(128) {
            if started.elapsed() >= Duration::from_secs(2) {
                return Ok(String::new());
            }
            if remaining == 0 {
                break;
            }
            let part = ranges.GetElement(index)?.GetText(remaining)?.to_string();
            // A caret without selected text produces an empty range.
            if part.is_empty() {
                continue;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            remaining = (remaining - part.encode_utf16().count() as i32).max(0);
            text.push_str(&part);
        }
        Ok(text)
    }
}
