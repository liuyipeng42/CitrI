use super::{
    transaction::{
        apply_transaction, backup_before_change, broadcast_restored, capture_unlocked, stores,
    },
    webdav::{DavClient, RemoteRead, UploadResult},
};
use crate::{
    data,
    history::merge_history,
    models::{DataOperationResult, SyncPreferences},
    state::AppState,
};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::{Emitter, Manager};

pub(super) fn emit_status(
    app: &tauri::AppHandle,
    state: &AppState,
    busy: bool,
    message: String,
    error: String,
) {
    let status = state.sync.set_status(busy, message, error);
    let _ = app.emit_to("settings", "sync-status", status);
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
        if pending {
            state.sync.request_sync();
        }
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

pub(super) async fn run_sync(
    app: &tauri::AppHandle,
    state: &AppState,
    preferences: Option<SyncPreferences>,
) -> Result<DataOperationResult, String> {
    let _operation = state
        .sync
        .operation
        .try_lock()
        .map_err(|_| "正在执行其他数据操作，请稍后重试")?;
    if let Some(preferences) = preferences {
        state.sync.save_preferences(preferences)?;
    }
    state.sync.take_pending();
    emit_status(app, state, true, "正在同步…".into(), String::new());
    let result = synchronize(app, state).await;
    match &result {
        Ok(result) => emit_status(app, state, false, result.message.clone(), String::new()),
        Err(error) => emit_status(app, state, false, String::new(), error.clone()),
    }
    state.sync.wake.notify_one();
    result
}

const AUTO_SYNC_DEBOUNCE: Duration = Duration::from_millis(800);

pub fn start_auto_sync(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            if state.is_quitting() {
                break;
            }
            let preferences = state.sync.preferences();
            if preferences.url.is_empty()
                || state.sync_editing.load(Ordering::Acquire)
                || state.sync.status().busy
                || !state.sync.has_pending()
            {
                state.sync.wake.notified().await;
                continue;
            }
            tokio::select! {
                _ = tokio::time::sleep(AUTO_SYNC_DEBOUNCE) => {}
                _ = state.sync.wake.notified() => continue,
            }
            if state.is_quitting() {
                break;
            }
            let preferences = state.sync.preferences();
            if preferences.url.is_empty()
                || state.sync_editing.load(Ordering::Acquire)
                || state.sync.status().busy
                || !state.sync.take_pending()
            {
                continue;
            }
            let _ = run_sync(&app, &state, None).await;
        }
    });
}
