use crate::{
    commands::require_window,
    data::{self, DataSnapshot, DavClient, MAX_ARCHIVE_BYTES, RemoteRead, UploadResult},
    history::{merge_history, now_millis},
    models::{DataInfo, DataOperationResult, SyncPreferences},
    state::AppState,
    ui,
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

struct DataStores<'a> {
    settings: &'a crate::settings::SettingsStore,
    history: &'a crate::history::HistoryStore,
    sync: &'a data::SyncStore,
}

fn stores(state: &AppState) -> DataStores<'_> {
    DataStores {
        settings: &state.settings,
        history: &state.history,
        sync: &state.sync,
    }
}

fn capture_unlocked(state: &DataStores<'_>) -> Result<DataSnapshot, String> {
    Ok(DataSnapshot {
        settings: state.settings.get(),
        history: state.history.snapshot()?,
        settings_modified_at: state.sync.modified_at(),
        includes_api_keys: true,
    })
}

fn capture(state: &AppState) -> Result<DataSnapshot, String> {
    let _guard = state.lock_data()?;
    capture_unlocked(&stores(state))
}

fn backup_before_change(
    state: &DataStores<'_>,
    snapshot: &DataSnapshot,
) -> Result<PathBuf, String> {
    let directory = state.settings.directory().join("backups");
    let path = directory.join(format!(
        "citri-before-import-{}-{}.zip",
        now_millis(),
        uuid::Uuid::new_v4()
    ));
    data::write_atomic(&path, &data::encode_archive(snapshot, true)?)?;
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
fn apply_transaction(
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

fn broadcast_restored(app: &tauri::AppHandle, state: &AppState, settings_changed: bool) {
    let settings = state.settings.get();
    state.selection.set_enabled(settings.enabled);
    if !settings.enabled {
        crate::selection::hide_toolbar(app);
    }
    if let Some(toggle) = state.tray_toggle() {
        let _ = toggle.set_checked(settings.enabled);
    }
    ui::broadcast_settings(app, &settings);
    if settings_changed {
        let _ = app.emit_to("settings", "settings-restored", &settings);
    }
    let _ = app.emit_to("settings", "history-changed", ());
    let _ = app.emit_to("settings", "data-changed", ());
}

fn emit_status(
    app: &tauri::AppHandle,
    state: &AppState,
    busy: bool,
    message: String,
    error: String,
) {
    let status = state.sync.set_status(busy, message, error);
    let _ = app.emit_to("settings", "sync-status", status);
}

#[tauri::command]
pub fn get_data_info(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<DataInfo, String> {
    require_window(&window, "settings")?;
    Ok(DataInfo {
        directory: state.settings.directory().to_string_lossy().into(),
        history_count: state.history.count().unwrap_or(0),
        preferences: state.sync.preferences(),
        status: state.sync.status(),
    })
}

#[tauri::command]
pub fn save_sync_preferences(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    preferences: SyncPreferences,
) -> Result<SyncPreferences, String> {
    require_window(&window, "settings")?;
    let _operation = state
        .sync
        .operation
        .try_lock()
        .map_err(|_| "正在执行数据操作，请完成后再修改连接设置")?;
    let saved = state.sync.save_preferences(preferences)?;
    emit_status(&app, &state, false, "同步设置已保存".into(), String::new());
    Ok(saved)
}

#[tauri::command]
pub fn set_sync_editing(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
    editing: bool,
) -> Result<(), String> {
    require_window(&window, "settings")?;
    state.sync_editing.store(editing, Ordering::Release);
    if !editing {
        state.sync.wake.notify_one();
    }
    Ok(())
}

#[tauri::command]
pub async fn export_data(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    include_api_keys: bool,
) -> Result<Option<DataOperationResult>, String> {
    require_window(&window, "settings")?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title("导出 CitrI 数据")
        .add_filter("ZIP 压缩包", &["zip"])
        .set_file_name(format!("CitrI-backup-{}.zip", now_millis()))
        .save_file(move |path| {
            let _ = sender.send(path);
        });
    let Some(path) = receiver.await.map_err(|_| "文件选择器已关闭")? else {
        return Ok(None);
    };
    let mut path = path.into_path().map_err(|e| e.to_string())?;
    if path.extension().is_none() {
        path.set_extension("zip");
    }
    let _operation = state
        .sync
        .operation
        .try_lock()
        .map_err(|_| "正在执行其他数据操作，请稍后重试")?;
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = capture(&app.state::<AppState>())?;
        let bytes = data::encode_archive(&snapshot, include_api_keys)?;
        data::write_atomic(&path, &bytes)?;
        Ok(Some(DataOperationResult {
            message: "已导出设置和历史记录".into(),
            history_count: snapshot.history.entries.len(),
            path: Some(path.to_string_lossy().into()),
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn read_archive_file(path: &Path) -> Result<DataSnapshot, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("无法打开备份：{e}"))?
        .take(MAX_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取备份：{e}"))?;
    data::decode_archive(&bytes)
}

fn restore_snapshot(
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

#[tauri::command]
pub async fn import_data(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<DataOperationResult>, String> {
    require_window(&window, "settings")?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title("导入 CitrI 数据")
        .add_filter("ZIP 压缩包", &["zip"])
        .pick_file(move |path| {
            let _ = sender.send(path);
        });
    let Some(path) = receiver.await.map_err(|_| "文件选择器已关闭")? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let _operation = state
        .sync
        .operation
        .try_lock()
        .map_err(|_| "正在执行其他数据操作，请稍后重试")?;
    let restore_app = app.clone();
    let (backup, count) = tauri::async_runtime::spawn_blocking(move || {
        let incoming = read_archive_file(&path)?;
        let state = restore_app.state::<AppState>();
        let _guard = state.lock_data()?;
        restore_snapshot(&stores(&state), incoming)
    })
    .await
    .map_err(|e| e.to_string())??;
    broadcast_restored(&app, &state, true);
    Ok(Some(DataOperationResult {
        message: "已导入设置并合并历史记录，导入前数据已自动备份".into(),
        history_count: count,
        path: Some(backup.to_string_lossy().into()),
    }))
}

#[tauri::command]
pub async fn test_webdav(
    window: WebviewWindow,
    preferences: SyncPreferences,
) -> Result<String, String> {
    require_window(&window, "settings")?;
    DavClient::new(preferences)?.test().await?;
    Ok("WebDAV 目录可访问".into())
}

async fn synchronize(
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<DataOperationResult, String> {
    let preferences = state.sync.preferences();
    let client = DavClient::new(preferences.clone())?;
    for _ in 0..4 {
        if state.sync_editing.load(Ordering::Acquire) {
            return Err("正在编辑设置，请完成编辑后再同步".into());
        }
        let probe = {
            let _guard = state.lock_data()?;
            state.sync.probe_etag(state.history.revision())
        };
        let remote = match client.read(probe.as_deref()).await? {
            RemoteRead::Unchanged => {
                let _guard = state.lock_data()?;
                if state.sync.probe_etag(state.history.revision()) != probe {
                    continue;
                }
                state.sync.checked();
                return Ok(DataOperationResult {
                    message: "数据已是最新，无需重复上传".into(),
                    history_count: state.history.count()?,
                    path: None,
                });
            }
            RemoteRead::Missing => None,
            RemoteRead::Archive(remote) => Some(remote),
        };
        let remote_etag = remote.as_ref().map(|remote| remote.etag.clone());
        let prepare_app = app.clone();
        let include_keys = preferences.include_api_keys;
        let (local, mut merged, local_revision, bytes) =
            tauri::async_runtime::spawn_blocking(move || {
                let state = prepare_app.state::<AppState>();
                let (local, local_revision) = {
                    let _guard = state.lock_data()?;
                    (capture_unlocked(&stores(&state))?, state.history.revision())
                };
                let decoded = remote
                    .as_ref()
                    .map(|remote| data::decode_archive(&remote.bytes))
                    .transpose()?;
                let merged = if let Some(remote) = &decoded {
                    data::merge_snapshots(&local, remote, state.sync.first_sync())
                } else {
                    local.clone()
                };
                let unchanged = decoded
                    .as_ref()
                    .is_some_and(|remote| data::same_snapshot(&merged, remote, include_keys));
                let bytes = if unchanged {
                    None
                } else {
                    Some(data::encode_archive(&merged, include_keys)?)
                };
                Ok::<_, String>((local, merged, local_revision, bytes))
            })
            .await
            .map_err(|e| e.to_string())??;
        if state.sync_editing.load(Ordering::Acquire) {
            return Err("正在编辑设置，请完成编辑后再同步".into());
        }
        let etag = if let Some(bytes) = bytes {
            match client.write(bytes, remote_etag.as_deref()).await? {
                UploadResult::Conflict => continue,
                UploadResult::Written(etag) => etag,
            }
        } else {
            remote_etag
        };
        let apply_app = app.clone();
        let (settings_changed, pending, count) = tauri::async_runtime::spawn_blocking(move || {
            let state = apply_app.state::<AppState>();
            let _guard = state.lock_data()?;
            let data_stores = stores(&state);
            let current = capture_unlocked(&data_stores)?;
            let settings_pending = data::settings_hash(&current.settings)
                != data::settings_hash(&local.settings)
                || state.sync_editing.load(Ordering::Acquire);
            let pending = settings_pending || state.history.revision() != local_revision;
            if settings_pending {
                merged.settings = current.settings.clone();
                merged.settings_modified_at = current.settings_modified_at;
            }
            if !merged.includes_api_keys {
                data::preserve_api_keys(&mut merged.settings, &current.settings);
            }
            merged.history = merge_history(
                &current.history,
                &merged.history,
                merged.settings.history_limit,
            );
            let settings_changed =
                data::settings_hash(&merged.settings) != data::settings_hash(&current.settings);
            if settings_changed || merged.history.cleared_at > current.history.cleared_at {
                backup_before_change(&data_stores, &current)?;
            }
            apply_transaction(&data_stores, &current, &merged, true)?;
            if !pending && let Some(etag) = etag {
                state.sync.remember_remote(etag, state.history.revision())?;
            }
            Ok::<_, String>((settings_changed, pending, merged.history.entries.len()))
        })
        .await
        .map_err(|e| e.to_string())??;
        broadcast_restored(app, state, settings_changed);
        return Ok(DataOperationResult {
            message: if pending {
                "同步完成；同步期间的新内容将在下次同步上传".into()
            } else {
                "设置和历史记录已同步".into()
            },
            history_count: count,
            path: None,
        });
    }
    Err("其他设备正在修改云端数据，已停止覆盖，请稍后再次同步".into())
}

async fn run_sync(app: &tauri::AppHandle, state: &AppState) -> Result<DataOperationResult, String> {
    let _operation = state
        .sync
        .operation
        .try_lock()
        .map_err(|_| "正在执行其他数据操作，请稍后重试")?;
    emit_status(app, state, true, "正在同步…".into(), String::new());
    let result = synchronize(app, state).await;
    match &result {
        Ok(result) => emit_status(app, state, false, result.message.clone(), String::new()),
        Err(error) => emit_status(app, state, false, String::new(), error.clone()),
    }
    state.sync.wake.notify_one();
    result
}

#[tauri::command]
pub async fn sync_webdav(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<DataOperationResult, String> {
    require_window(&window, "settings")?;
    run_sync(&app, &state).await
}

#[tauri::command]
pub fn open_data_directory(
    window: WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    require_window(&window, "settings")?;
    let directory = state.settings.directory();
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    open::that_detached(directory).map_err(|e| e.to_string())
}

pub fn start_auto_sync(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_attempt: Option<Instant> = None;
        let mut previous = None;
        loop {
            let state = app.state::<AppState>();
            if state.is_quitting() {
                break;
            }
            let preferences = state.sync.preferences();
            if previous.as_ref() != Some(&preferences) {
                last_attempt = None;
                previous = Some(preferences.clone());
            }
            if !preferences.auto_sync || preferences.url.is_empty() {
                state.sync.wake.notified().await;
                continue;
            }
            let interval =
                Duration::from_secs(preferences.interval_minutes.clamp(1, 1440) as u64 * 60);
            let wait = last_attempt.map_or(Duration::ZERO, |attempt| {
                interval.saturating_sub(attempt.elapsed())
            });
            if !wait.is_zero() {
                tokio::select! {_=tokio::time::sleep(wait)=>{},_=state.sync.wake.notified()=>{}}
                continue;
            }
            if state.sync_editing.load(Ordering::Acquire) || state.sync.status().busy {
                state.sync.wake.notified().await;
                continue;
            }
            last_attempt = Some(Instant::now());
            let _ = run_sync(&app, &state).await;
        }
    });
}

#[cfg(test)]
mod tests {
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
}
