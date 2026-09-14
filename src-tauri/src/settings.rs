use crate::constants::clamp_action_window_size;
use crate::models::{ApiProfile, PublicSettings, Settings};
use atomic_write_file::AtomicWriteFile;
use std::{fs, io::Write, path::PathBuf, sync::RwLock};

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
        let value = value.sanitized();
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

    fn persist_value(&self, value: &Settings) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_vec_pretty(value)?;
        let mut file = AtomicWriteFile::open(&self.path)?;
        file.write_all(&contents)?;
        file.write_all(b"\n")?;
        file.commit()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
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

    #[test]
    fn round_trips_settings_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        let mut settings = store.get();
        settings.compact = true;
        store.save(settings).unwrap();
        assert!(path.exists());
        let loaded = SettingsStore::load(path);
        assert!(loaded.read(|settings| settings.compact));
        assert_eq!(loaded.public().compact, loaded.get().compact);
        assert_eq!(loaded.active_profile(), loaded.get().active_profile());
    }

    #[test]
    fn remembers_action_window_size_only_when_enabled() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = SettingsStore::load(path.clone());

        store.save_action_window_size(720, 560).unwrap();
        assert_eq!(store.read(|settings| settings.action_window_width), 500);

        let mut settings = store.get();
        settings.remember_action_window_size = true;
        store.save(settings).unwrap();
        store.save_action_window_size(720, 560).unwrap();

        let loaded = SettingsStore::load(path);
        assert_eq!(loaded.read(|settings| settings.action_window_width), 720);
        assert_eq!(loaded.read(|settings| settings.action_window_height), 560);
    }
}
