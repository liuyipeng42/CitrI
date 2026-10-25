use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum HistoryStatus {
    Success,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntry {
    pub sync_id: String,
    #[ts(type = "number")]
    pub revived_at: u64,
    #[ts(type = "number")]
    pub id: u64,
    #[ts(type = "number")]
    pub created_at: u64,
    pub action_name: String,
    pub model: String,
    pub selected_text: String,
    pub prompt: String,
    pub system_prompt: String,
    pub result: String,
    pub status: HistoryStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistorySummary {
    #[ts(type = "number")]
    pub id: u64,
    #[ts(type = "number")]
    pub created_at: u64,
    pub action_name: String,
    pub preview: String,
    pub status: HistoryStatus,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryPage {
    pub entries: Vec<HistorySummary>,
    pub total: usize,
    pub filtered_total: usize,
}
