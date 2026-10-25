use super::{ActionItem, PublicSettings};
use serde::Serialize;
use std::time::{Duration, Instant};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SelectionPayload {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActionWindowSession {
    pub action: ActionItem,
    pub selected_text: String,
    pub pinned: bool,
    pub frontend_ready: bool,
    pub auto_close_armed: bool,
    pub last_window_size: Option<(u32, u32)>,
    pub(crate) native_interaction_until: Option<Instant>,
}

impl ActionWindowSession {
    pub fn mark_frontend_ready(&mut self, focused: bool) {
        self.frontend_ready = true;
        self.auto_close_armed = focused;
    }

    pub fn on_focus_changed(&mut self, focused: bool) -> bool {
        if focused {
            self.native_interaction_until = None;
            if self.frontend_ready {
                self.auto_close_armed = true;
            }
            return false;
        }
        self.auto_close_armed && !self.pinned
    }

    pub fn should_close_after_blur(&self, focused: bool, cursor_near_window: bool) -> bool {
        !focused
            && !cursor_near_window
            && !self.native_interaction_is_active()
            && self.auto_close_armed
            && !self.pinned
    }

    pub fn begin_native_interaction(&mut self) {
        self.native_interaction_until = Some(Instant::now() + Duration::from_secs(2));
    }

    pub fn note_native_window_change(&mut self) {
        self.native_interaction_until = Some(Instant::now() + Duration::from_millis(500));
    }

    pub fn note_window_size(&mut self, width: u32, height: u32) {
        // Minimizing can report a zero client area. Preserve the restore size.
        if width == 0 || height == 0 {
            return;
        }
        self.last_window_size = Some((width, height));
        self.note_native_window_change();
    }

    fn native_interaction_is_active(&self) -> bool {
        self.native_interaction_until
            .is_some_and(|deadline| Instant::now() <= deadline)
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionSession {
    pub action: ActionItem,
    pub selected_text: String,
    pub settings: PublicSettings,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolbarState {
    pub settings: PublicSettings,
    pub selection: Option<SelectionPayload>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlatformInfo {
    pub platform: String,
    pub display_protocol: String,
}
