use crate::{
    history::{HistorySnapshot, merge_history},
    models::Settings,
};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct DataSnapshot {
    pub settings: Settings,
    pub history: HistorySnapshot,
    pub settings_modified_at: u64,
    pub includes_api_keys: bool,
}

pub fn portable_settings(settings: &Settings) -> Settings {
    let mut settings = settings.clone();
    let defaults = Settings::default();
    settings.action_window_width = defaults.action_window_width;
    settings.action_window_height = defaults.action_window_height;
    settings
}

pub fn settings_hash(settings: &Settings) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&portable_settings(settings)).unwrap_or_default())
    )
}

pub fn preserve_api_keys(imported: &mut Settings, current: &Settings) {
    for profile in &mut imported.api_profiles {
        if let Some(local) = current
            .api_profiles
            .iter()
            .find(|local| local.id == profile.id && local.api_url == profile.api_url)
        {
            profile.api_key.clone_from(&local.api_key);
        }
    }
}

pub fn merge_snapshots(
    local: &DataSnapshot,
    remote: &DataSnapshot,
    first_sync: bool,
) -> DataSnapshot {
    let remote_wins = first_sync
        || (remote.settings_modified_at, settings_hash(&remote.settings))
            > (local.settings_modified_at, settings_hash(&local.settings));
    let source = if remote_wins { remote } else { local };
    let mut settings = source.settings.clone();
    if !source.includes_api_keys {
        preserve_api_keys(&mut settings, &local.settings);
    }
    let history = merge_history(&local.history, &remote.history, settings.history_limit);
    DataSnapshot {
        settings,
        history,
        settings_modified_at: source.settings_modified_at,
        includes_api_keys: source.includes_api_keys,
    }
}

pub fn same_snapshot(left: &DataSnapshot, right: &DataSnapshot, include_keys: bool) -> bool {
    if right.includes_api_keys != include_keys
        || left.settings_modified_at != right.settings_modified_at
        || left.history.cleared_at != right.history.cleared_at
        || left.history.entries.len() != right.history.entries.len()
    {
        return false;
    }
    let mut a = portable_settings(&left.settings);
    let mut b = portable_settings(&right.settings);
    if !include_keys {
        for profile in a.api_profiles.iter_mut().chain(b.api_profiles.iter_mut()) {
            profile.api_key.clear();
        }
    }
    if a != b {
        return false;
    }
    let remote: std::collections::HashMap<_, _> = right
        .history
        .entries
        .iter()
        .map(|entry| (&entry.sync_id, entry))
        .collect();
    left.history.entries.iter().all(|a| {
        remote.get(&a.sync_id).is_some_and(|b| {
            a.created_at == b.created_at
                && a.revived_at == b.revived_at
                && a.action_name == b.action_name
                && a.model == b.model
                && a.selected_text == b.selected_text
                && a.prompt == b.prompt
                && a.system_prompt == b.system_prompt
                && a.result == b.result
                && a.status == b.status
                && a.error == b.error
        })
    })
}
