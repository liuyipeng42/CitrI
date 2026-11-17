use super::{snapshot::settings_hash, webdav::dav_directory_url};
use crate::{
    history::now_millis,
    models::{Settings, SyncPreferences, SyncStatus},
    storage::write_atomic,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct SyncConfig {
    preferences: SyncPreferences,
    settings_hash: String,
    settings_modified_at: u64,
    last_synced_at: u64,
    remote_etag: String,
    synced_settings_hash: String,
    synced_history_revision: u64,
    synced_api_keys: bool,
}

pub struct SyncStore {
    path: PathBuf,
    value: Mutex<SyncConfig>,
    status: Mutex<SyncStatus>,
    pending: AtomicBool,
    pub operation: tokio::sync::Mutex<()>,
    pub wake: tokio::sync::Notify,
}

impl SyncStore {
    pub fn load(path: PathBuf, settings: &Settings, modified_at: u64) -> Self {
        let mut value: SyncConfig = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let hash = settings_hash(settings);
        if value.settings_hash != hash {
            value.settings_hash = hash;
            value.settings_modified_at = modified_at;
        }
        let status = SyncStatus {
            last_synced_at: value.last_synced_at,
            ..SyncStatus::default()
        };
        Self {
            path,
            value: Mutex::new(value),
            status: Mutex::new(status),
            pending: AtomicBool::new(true),
            operation: tokio::sync::Mutex::new(()),
            wake: tokio::sync::Notify::new(),
        }
    }

    pub fn preferences(&self) -> SyncPreferences {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .preferences
            .clone()
    }
    pub fn status(&self) -> SyncStatus {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        status.last_synced_at = self
            .value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at;
        status
    }
    pub fn modified_at(&self) -> u64 {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .settings_modified_at
    }
    pub fn first_sync(&self) -> bool {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at
            == 0
    }

    fn persist(&self, value: &SyncConfig) -> Result<(), String> {
        let mut contents = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        contents.push(b'\n');
        write_atomic(&self.path, &contents)
    }

    pub fn save_preferences(
        &self,
        mut preferences: SyncPreferences,
    ) -> Result<SyncPreferences, String> {
        preferences.url = preferences.url.trim().to_string();
        preferences.username = preferences.username.trim().to_string();
        if preferences.url.len() > 2000
            || preferences.username.len() > 1000
            || preferences.password.len() > 8000
        {
            return Err("WebDAV 配置过长".into());
        }
        if !preferences.url.is_empty() {
            dav_directory_url(&preferences.url)?;
        }
        let mut current = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        let mut next = current.clone();
        let connection_changed = next.preferences.url != preferences.url
            || next.preferences.username != preferences.username;
        // A different server is a new synchronization relationship.
        if connection_changed {
            next.last_synced_at = 0;
            next.remote_etag.clear();
            next.synced_settings_hash.clear();
            next.synced_history_revision = 0;
        }
        let changed = connection_changed || next.preferences != preferences;
        next.preferences = preferences.clone();
        self.persist(&next)?;
        *current = next;
        if changed && !preferences.url.is_empty() {
            self.pending.store(true, Ordering::Release);
        }
        self.wake.notify_one();
        Ok(preferences)
    }

    pub fn note_settings(&self, settings: &Settings) -> Result<(), String> {
        let hash = settings_hash(settings);
        let mut current = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        if current.settings_hash == hash {
            return Ok(());
        }
        let mut next = current.clone();
        next.settings_hash = hash;
        next.settings_modified_at =
            now_millis().max(current.settings_modified_at.saturating_add(1));
        self.persist(&next)?;
        *current = next;
        drop(current);
        self.request_sync();
        Ok(())
    }

    pub fn mark_applied(
        &self,
        settings: &Settings,
        modified_at: u64,
        synced: bool,
    ) -> Result<(), String> {
        let mut current = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        let mut next = current.clone();
        next.settings_hash = settings_hash(settings);
        next.settings_modified_at = modified_at;
        if synced {
            next.last_synced_at = now_millis();
        }
        self.persist(&next)?;
        *current = next;
        Ok(())
    }

    pub fn probe_etag(&self, history_revision: u64) -> Option<String> {
        let value = self.value.lock().unwrap_or_else(|e| e.into_inner());
        (!value.remote_etag.is_empty()
            && value.synced_settings_hash == value.settings_hash
            && value.synced_history_revision == history_revision
            && value.synced_api_keys == value.preferences.include_api_keys)
            .then(|| value.remote_etag.clone())
    }

    pub fn remember_remote(&self, etag: String, history_revision: u64) -> Result<(), String> {
        let mut value = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        let mut next = value.clone();
        next.remote_etag = etag;
        next.synced_settings_hash = next.settings_hash.clone();
        next.synced_history_revision = history_revision;
        next.synced_api_keys = next.preferences.include_api_keys;
        self.persist(&next)?;
        *value = next;
        Ok(())
    }

    pub fn checked(&self) {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at = now_millis();
    }

    pub fn request_sync(&self) {
        self.pending.store(true, Ordering::Release);
        self.wake.notify_one();
    }

    pub fn has_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    pub fn take_pending(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    pub fn set_status(&self, busy: bool, message: String, error: String) -> SyncStatus {
        let last_synced_at = self
            .value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at;
        let status = SyncStatus {
            busy,
            last_synced_at,
            message,
            error,
        };
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = status.clone();
        status
    }
}
