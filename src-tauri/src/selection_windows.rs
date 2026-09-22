use std::sync::Mutex;
use windows::{
    Win32::{
        Foundation::POINT,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
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
pub(super) fn read_selection(point: (f64, f64)) -> Result<String> {
    static READER: Mutex<()> = Mutex::new(());
    let Ok(_reader) = READER.try_lock() else {
        return Ok(String::new());
    };
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let _apartment = ComApartment;
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return Ok(String::new());
        }
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
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
                if GetForegroundWindow() != foreground {
                    return Ok(String::new());
                }
                // Never read password controls, including through their parent.
                if element.CurrentIsPassword()?.as_bool() {
                    break;
                }
                if let Ok(text) = element_selection(&element)
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

unsafe fn element_selection(element: &IUIAutomationElement) -> Result<String> {
    unsafe {
        let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId)?;
        let ranges = pattern.GetSelection()?;
        let mut text = String::new();
        let mut remaining = 200_000;
        for index in 0..ranges.Length()?.min(128) {
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
