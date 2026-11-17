mod db;
mod queries;
mod store;

use crate::{constants::clamp_history_limit, models::HistoryEntry};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
pub use store::HistoryStore;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistorySnapshot {
    pub entries: Vec<HistoryEntry>,
    pub cleared_at: u64,
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn merge_history(
    local: &HistorySnapshot,
    remote: &HistorySnapshot,
    limit: u32,
) -> HistorySnapshot {
    let cleared_at = local.cleared_at.max(remote.cleared_at);
    let mut entries: HashMap<&str, &HistoryEntry> = HashMap::new();
    for entry in local.entries.iter().chain(&remote.entries) {
        if entry.created_at <= cleared_at && entry.revived_at <= cleared_at {
            continue;
        }
        entries
            .entry(&entry.sync_id)
            .and_modify(|existing| {
                if entry.revived_at > existing.revived_at {
                    *existing = entry;
                }
            })
            .or_insert(entry);
    }
    let mut entries: Vec<_> = entries.into_values().collect();
    entries.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.sync_id.cmp(&a.sync_id))
    });
    entries.truncate(clamp_history_limit(limit));
    HistorySnapshot {
        entries: entries.into_iter().cloned().collect(),
        cleared_at,
    }
}

#[cfg(test)]
mod tests;
