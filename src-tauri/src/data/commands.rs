use super::{
    sync::{emit_status, run_sync},
    transaction::{broadcast_restored, capture, read_archive_file, restore_snapshot, stores},
    webdav::DavClient,
};
use crate::{
    commands::require_window,
    data,
    history::now_millis,
    models::{DataInfo, DataOperationResult, SyncPreferences},
    state::AppState,
    storage::write_atomic,
};
use std::{fs, sync::atomic::Ordering};
use tauri::{Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

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
        write_atomic(&path, &bytes)?;
        Ok(Some(DataOperationResult {
            message: "已导出设置和历史记录".into(),
            history_count: snapshot.history.entries.len(),
            path: Some(path.to_string_lossy().into()),
        }))
    })
    .await
    .map_err(|e| e.to_string())?
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
