use super::*;
use crate::{data::tests::sample, history::HistoryStore, settings::SettingsStore};

fn store(directory: &Path) -> (SettingsStore, HistoryStore, data::SyncStore) {
    let settings = SettingsStore::load(directory.join("settings.json"));
    settings.save(sample().settings).unwrap();
    let sync = data::SyncStore::load(directory.join("sync.json"), &settings.get(), 100);
    (
        settings,
        HistoryStore::load(directory.join("history.db"), 100).unwrap(),
        sync,
    )
}

fn refs(owned: &(SettingsStore, HistoryStore, data::SyncStore)) -> DataStores<'_> {
    DataStores {
        settings: &owned.0,
        history: &owned.1,
        sync: &owned.2,
    }
}

#[test]
fn importing_restores_cleared_history_preserves_keys_and_creates_recovery_backups() {
    let directory = tempfile::tempdir().unwrap();
    let owned = store(directory.path());
    let state = refs(&owned);
    state.history.clear().unwrap();
    let archive = data::encode_archive(&sample(), false).unwrap();
    let mut incoming = data::decode_archive(&archive).unwrap();
    incoming.settings.font_size = 110;
    let (backup, count) = restore_snapshot(&state, incoming.clone()).unwrap();
    assert_eq!(count, 1);
    assert_eq!(state.settings.get().font_size, 110);
    assert_eq!(state.settings.get().api_profiles[0].api_key, "test-api-key");
    let recovery = read_archive_file(&backup).unwrap();
    assert_eq!(recovery.settings.font_size, 100);
    assert!(recovery.history.entries.is_empty());
    assert_eq!(recovery.settings.api_profiles[0].api_key, "test-api-key");
    assert_eq!(
        merge_history(&state.history.snapshot().unwrap(), &sample().history, 100)
            .entries
            .len(),
        1
    );
    for _ in 0..6 {
        restore_snapshot(&state, incoming.clone()).unwrap();
    }
    assert_eq!(
        fs::read_dir(directory.path().join("backups"))
            .unwrap()
            .count(),
        5
    );
    assert_eq!(state.history.get().unwrap().len(), 1);
}

#[test]
fn failed_history_write_rolls_back_settings_and_preserves_recovery_archive() {
    let directory = tempfile::tempdir().unwrap();
    let owned = store(directory.path());
    let state = refs(&owned);
    let before = capture_unlocked(&state).unwrap();
    let backup = backup_before_change(&state, &before).unwrap();
    state.history.make_read_only();
    let mut incoming = before.clone();
    incoming.settings.compact = true;
    assert!(apply_transaction(&state, &before, &incoming, false).is_err());
    assert!(!state.settings.get().compact);
    assert!(
        !SettingsStore::load(directory.path().join("settings.json"))
            .get()
            .compact
    );
    assert!(read_archive_file(&backup).is_ok());
}

#[test]
fn merged_remote_ids_do_not_collide_with_pending_local_requests() {
    let directory = tempfile::tempdir().unwrap();
    let owned = store(directory.path());
    let state = refs(&owned);
    let entry = sample().history.entries[0].clone();
    state
        .history
        .append(state.history.begin().unwrap(), entry.clone())
        .unwrap();
    let pending = state.history.begin().unwrap();
    let mut remote = sample().history;
    remote.entries[0].sync_id = "remote".into();
    remote.entries[0].created_at = 51;
    let merged = merge_history(&state.history.snapshot().unwrap(), &remote, 100);
    state.history.replace(merged, 100).unwrap();
    let mut pending_entry = entry;
    pending_entry.sync_id = "pending".into();
    state.history.append(pending, pending_entry).unwrap();
    let entries = state.history.get().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
}
