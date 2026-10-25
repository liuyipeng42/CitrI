use crate::constants::clamp_action_window_size;
use crate::models::{ApiProfile, PublicSettings, Settings};
use crate::storage::write_atomic;
use std::{fs, path::PathBuf, sync::RwLock};

pub struct SettingsStore {
    path: PathBuf,
    value: RwLock<Settings>,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let value = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str::<Settings>(&contents).ok())
            .unwrap_or_default()
            .sanitized();
        Self {
            path,
            value: RwLock::new(value),
        }
    }

    pub fn get(&self) -> Settings {
        self.read(Clone::clone)
    }

    pub fn directory(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf()
    }

    pub fn modified_at(&self) -> u64 {
        fs::metadata(&self.path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_millis() as u64)
    }

    pub fn read<T>(&self, reader: impl FnOnce(&Settings) -> T) -> T {
        let value = self
            .value
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reader(&value)
    }

    pub fn public(&self) -> PublicSettings {
        self.read(|settings| PublicSettings::from(settings))
    }

    pub fn active_profile(&self) -> ApiProfile {
        self.read(|settings| settings.active_profile())
    }

    pub fn save(&self, mut value: Settings) -> Result<Settings, String> {
        // Serialize the disk write and memory update with window-size saves.
        let mut current = self
            .value
            .write()
            .map_err(|_| "设置存储已损坏".to_string())?;
        value.action_window_width = current.action_window_width;
        value.action_window_height = current.action_window_height;
        if *current == value {
            return Ok(value);
        }
        let value = value.sanitized();
        if *current == value {
            return Ok(value);
        }
        self.persist_value(&value)
            .map_err(|error| format!("无法保存设置：{error}"))?;
        *current = value.clone();
        Ok(value)
    }

    pub fn save_action_window_size(&self, width: u32, height: u32) -> Result<(), String> {
        let mut current = self
            .value
            .write()
            .map_err(|_| "设置存储已损坏".to_string())?;
        if !current.remember_action_window_size {
            return Ok(());
        }

        let (width, height) = clamp_action_window_size(width, height);
        if (current.action_window_width, current.action_window_height) == (width, height) {
            return Ok(());
        }

        let mut next = current.clone();
        next.action_window_width = width;
        next.action_window_height = height;
        self.persist_value(&next)
            .map_err(|error| format!("无法保存窗口大小：{error}"))?;
        *current = next;
        Ok(())
    }

    fn persist_value(&self, value: &Settings) -> Result<(), String> {
        let mut contents = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
        contents.push(b'\n');
        write_atomic(&self.path, &contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_settings_form_preserves_latest_window_size() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        let mut settings = store.get();
        settings.remember_action_window_size = true;
        let mut stale_form = store.save(settings).unwrap();
        store.save_action_window_size(800, 600).unwrap();
        stale_form.compact = true;
        store.save(stale_form).unwrap();
        let saved = SettingsStore::load(path).get();
        assert!(saved.compact);
        assert_eq!(
            (saved.action_window_width, saved.action_window_height),
            (800, 600)
        );
    }
}
