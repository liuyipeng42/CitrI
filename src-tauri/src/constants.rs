pub const TOOLBAR_DEFAULT_WIDTH: u32 = 350;
pub const TOOLBAR_DEFAULT_HEIGHT: u32 = 43;

pub const ACTION_WINDOW_DEFAULT_WIDTH: u32 = 500;
pub const ACTION_WINDOW_DEFAULT_HEIGHT: u32 = 400;
pub const ACTION_WINDOW_MIN_WIDTH: u32 = 300;
pub const ACTION_WINDOW_MIN_HEIGHT: u32 = 200;
pub const ACTION_WINDOW_MAX_WIDTH: u32 = 3840;
pub const ACTION_WINDOW_MAX_HEIGHT: u32 = 2160;

pub const MAX_HISTORY_LIMIT: u32 = 1000;

pub fn clamp_action_window_size(width: u32, height: u32) -> (u32, u32) {
    (
        width.clamp(ACTION_WINDOW_MIN_WIDTH, ACTION_WINDOW_MAX_WIDTH),
        height.clamp(ACTION_WINDOW_MIN_HEIGHT, ACTION_WINDOW_MAX_HEIGHT),
    )
}

pub fn clamp_history_limit(limit: u32) -> usize {
    limit.min(MAX_HISTORY_LIMIT) as usize
}
