use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, Default)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SyncPreferences {
    pub url: String,
    pub username: String,
    pub password: String,
    pub include_api_keys: bool,
}

#[derive(Debug, Clone, Serialize, TS, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncStatus {
    pub busy: bool,
    #[ts(type = "number")]
    pub last_synced_at: u64,
    pub message: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DataInfo {
    pub directory: String,
    pub history_count: usize,
    pub preferences: SyncPreferences,
    pub status: SyncStatus,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DataOperationResult {
    pub message: String,
    pub history_count: usize,
    pub path: Option<String>,
}
