pub mod actions;
pub mod history;
pub mod settings;
pub mod targets;
pub mod toolbar;
pub mod window;

use tauri::WebviewWindow;

pub(crate) fn require_window(window: &WebviewWindow, expected: &str) -> Result<(), String> {
    let valid = if expected == "action" {
        window.label().starts_with("action-")
    } else {
        window.label() == expected
    };
    valid
        .then_some(())
        .ok_or_else(|| "当前窗口无权执行此操作".into())
}

#[cfg(test)]
mod tests;
