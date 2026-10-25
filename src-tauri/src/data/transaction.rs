use crate::{
    data::{self, DataSnapshot, MAX_ARCHIVE_BYTES},
    desktop::ui,
    history::{merge_history, now_millis},
    state::AppState,
    storage::write_atomic,
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tauri::Emitter;

pub(super) struct DataStores<'a> {
    settings: &'a crate::settings::SettingsStore,
    history: &'a crate::history::HistoryStore,
    sync: &'a data::SyncStore,
}

pub(super) fn stores(state: &AppState) -> DataStores<'_> {
    DataStores {
        settings: &state.settings,
        history: &state.history,
        sync: &state.sync,
    }
}

pub(super) fn capture_unlocked(state: &DataStores<'_>) -> Result<DataSnapshot, String> {
    Ok(DataSnapshot {
        settings: state.settings.get(),
        history: state.history.snapshot()?,
        settings_modified_at: state.sync.modified_at(),
        includes_api_keys: true,
    })
}

pub(super) fn capture(state: &AppState) -> Result<DataSnapshot, String> {
    let _guard = state.lock_data()?;
    capture_unlocked(&stores(state))
}

pub(super) fn backup_before_change(
    state: &DataStores<'_>,
    snapshot: &DataSnapshot,
) -> Result<PathBuf, String> {
    let directory = state.settings.directory().join("backups");
    let path = directory.join(format!(
        "citri-before-import-{}-{}.zip",
        now_millis(),
        uuid::Uuid::new_v4()
    ));
    write_atomic(&path, &data::encode_archive(snapshot, true)?)?;
    // Only prune this feature's own backups; user-selected exports are untouched.
    let mut backups: Vec<_> = fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("citri-before-import-")
                && entry.path().extension().is_some_and(|ext| ext == "zip")
        })
        .map(|entry| entry.path())
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(5);
    for old in backups.into_iter().take(excess) {
        let _ = fs::remove_file(old);
    }
    Ok(path)
}

// Call under the data lock. Disk failures roll back the other store as well.
pub(super) fn apply_transaction(
    state: &DataStores<'_>,
    before: &DataSnapshot,
    incoming: &DataSnapshot,
    synced: bool,
) -> Result<(), String> {
    let apply = || -> Result<(), String> {
        let settings = state.settings.save(incoming.settings.clone())?;
        state
            .history
            .replace(incoming.history.clone(), settings.history_limit)?;
        state
            .sync
            .mark_applied(&settings, incoming.settings_modified_at, synced)?;
        Ok(())
    };
    if let Err(error) = apply() {
        let settings_rollback = state.settings.save(before.settings.clone());
        let history_rollback = state
            .history
            .replace(before.history.clone(), before.settings.history_limit);
        if settings_rollback.is_err() || history_rollback.is_err() {
            return Err(format!(
                "{error}；恢复当前数据失败，请使用数据目录 backups 中的自动备份"
            ));
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn broadcast_restored(app: &tauri::AppHandle, state: &AppState, settings_changed: bool) {
    let settings = state.settings.get();
    ui::apply_settings(app, &settings);
    if settings_changed {
        let _ = app.emit_to("settings", "settings-restored", &settings);
    }
    let _ = app.emit_to("settings", "history-changed", ());
    let _ = app.emit_to("settings", "data-changed", ());
}

pub(super) fn read_archive_file(path: &Path) -> Result<DataSnapshot, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("无法打开备份：{e}"))?
        .take(MAX_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取备份：{e}"))?;
    data::decode_archive(&bytes)
}

pub(super) fn restore_snapshot(
    state: &DataStores<'_>,
    mut incoming: DataSnapshot,
) -> Result<(PathBuf, usize), String> {
    let before = capture_unlocked(state)?;
    let backup = backup_before_change(state, &before)?;
    if !incoming.includes_api_keys {
        data::preserve_api_keys(&mut incoming.settings, &before.settings);
    }
    let revived_at = now_millis().max(before.history.cleared_at.saturating_add(1));
    for entry in &mut incoming.history.entries {
        entry.revived_at = revived_at;
    }
    incoming.history = merge_history(
        &before.history,
        &incoming.history,
        incoming.settings.history_limit,
    );
    incoming.settings_modified_at = now_millis().max(before.settings_modified_at.saturating_add(1));
    apply_transaction(state, &before, &incoming, false)?;
    Ok((backup, incoming.history.entries.len()))
}

#[cfg(test)]
#[path = "tests/transaction.rs"]
mod tests;
