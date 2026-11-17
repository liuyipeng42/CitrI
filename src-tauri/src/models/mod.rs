mod actions;
mod history;
mod sanitize;
mod settings;
mod sync;
mod windows;

pub use actions::ActionItem;
pub use history::{HistoryEntry, HistoryPage, HistoryStatus, HistorySummary};
pub use settings::{ApiProfile, FilterMode, PublicSettings, Settings};
pub use sync::{DataInfo, DataOperationResult, SyncPreferences, SyncStatus};
pub use windows::{
    ActionSession, ActionWindowSession, PlatformInfo, SelectionPayload, ToolbarState,
};

#[cfg(test)]
mod tests;
