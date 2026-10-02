use crate::models::HistoryEntry;
use atomic_write_file::AtomicWriteFile;
use std::{fs, io::Write, path::PathBuf, sync::Mutex};

pub struct HistoryStore {
    path: PathBuf,
    value: Mutex<HistoryState>,
}

struct HistoryState {
    entries: Vec<HistoryEntry>,
    limit: usize,
    next_id: u64,
    generation: u64,
    load_error: Option<String>,
}

pub struct HistoryTicket {
    pub id: u64,
    generation: u64,
}

impl HistoryStore {
    pub fn load(path: PathBuf, limit: u32) -> Result<Self, String> {
        let loaded: Result<Vec<HistoryEntry>, String> = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|error| format!("无法读取历史记录：{error}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(format!("无法读取历史记录：{error}")),
        };
        // A damaged history file must not prevent the assistant from starting.
        // Preserve the file and suspend recording until the user clears it.
        let (mut entries, load_error) = match loaded {
            Ok(entries) => (entries, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.id));
        let next_id = entries.first().map_or(1, |entry| entry.id + 1);
        let limit = limit.min(1000) as usize;
        let needs_trim = entries.len() > limit;
        entries.truncate(limit);
        let store = Self {
            path,
            value: Mutex::new(HistoryState {
                entries,
                limit,
                next_id,
                generation: 0,
                load_error,
            }),
        };
        if needs_trim {
            store.persist(&store.value.lock().unwrap().entries)?;
        }
        Ok(store)
    }

    pub fn get(&self) -> Result<Vec<HistoryEntry>, String> {
        let value = self.value.lock().map_err(|_| "历史记录存储已损坏")?;
        if let Some(error) = &value.load_error {
            return Err(error.clone());
        }
        Ok(value.entries.clone())
    }

    pub fn begin(&self) -> Option<HistoryTicket> {
        let mut value = self.value.lock().ok()?;
        if value.limit == 0 || value.load_error.is_some() {
            return None;
        }
        let ticket = HistoryTicket {
            id: value.next_id,
            generation: value.generation,
        };
        value.next_id += 1;
        Some(ticket)
    }

    pub fn append(&self, ticket: HistoryTicket, mut entry: HistoryEntry) -> Result<bool, String> {
        let mut value = self.value.lock().map_err(|_| "历史记录存储已损坏")?;
        // Clearing or disabling history also discards requests still in flight.
        if ticket.generation != value.generation || value.limit == 0 {
            return Ok(false);
        }
        entry.id = ticket.id;
        let mut entries = value.entries.clone();
        entries.push(entry);
        // Concurrent requests can finish in a different order from their start.
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.id));
        entries.truncate(value.limit);
        self.persist(&entries)?;
        value.entries = entries;
        Ok(true)
    }

    pub fn set_limit(&self, limit: u32) -> Result<(), String> {
        let mut value = self.value.lock().map_err(|_| "历史记录存储已损坏")?;
        let limit = limit.min(1000) as usize;
        if value.entries.len() > limit {
            let entries = value.entries[..limit].to_vec();
            self.persist(&entries)?;
            value.entries = entries;
        }
        if limit == 0 {
            value.generation += 1;
        }
        value.limit = limit;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut value = self.value.lock().map_err(|_| "历史记录存储已损坏")?;
        self.persist(&[])?;
        value.entries.clear();
        value.generation += 1;
        value.load_error = None;
        Ok(())
    }

    fn persist(&self, entries: &[HistoryEntry]) -> Result<(), String> {
        let write = || -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = AtomicWriteFile::open(&self.path)?;
            file.write_all(&serde_json::to_vec_pretty(entries)?)?;
            file.write_all(b"\n")?;
            file.commit()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
            }
            Ok(())
        };
        write().map_err(|error| format!("无法保存历史记录：{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HistoryStatus;

    fn entry(status: HistoryStatus) -> HistoryEntry {
        HistoryEntry {
            id: 0,
            created_at: 123,
            action_name: "翻译".into(),
            model: "test-model".into(),
            selected_text: "hello".into(),
            prompt: "翻译 hello".into(),
            system_prompt: "助手".into(),
            result: "你好".into(),
            status,
            error: (status == HistoryStatus::Error).then(|| "连接失败".into()),
        }
    }

    #[test]
    fn retains_newest_requests_when_they_finish_out_of_order_and_survive_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let store = HistoryStore::load(path.clone(), 2).unwrap();
        let first = store.begin().unwrap();
        let second = store.begin().unwrap();
        let third = store.begin().unwrap();
        store.append(third, entry(HistoryStatus::Success)).unwrap();
        store
            .append(first, entry(HistoryStatus::Cancelled))
            .unwrap();
        store.append(second, entry(HistoryStatus::Error)).unwrap();
        let loaded = HistoryStore::load(path, 2).unwrap().get().unwrap();
        assert_eq!(
            loaded.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            [3, 2]
        );
        assert_eq!(loaded[0].result, "你好");
        assert_eq!(loaded[1].status, HistoryStatus::Error);
        assert_eq!(loaded[1].error.as_deref(), Some("连接失败"));
    }

    #[test]
    fn shrinking_and_disabling_are_persisted_and_invalidate_pending_requests() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let store = HistoryStore::load(path.clone(), 3).unwrap();
        for _ in 0..3 {
            store
                .append(store.begin().unwrap(), entry(HistoryStatus::Success))
                .unwrap();
        }
        store.set_limit(1).unwrap();
        assert_eq!(
            HistoryStore::load(path.clone(), 1)
                .unwrap()
                .get()
                .unwrap()
                .len(),
            1
        );
        let pending = store.begin().unwrap();
        store.set_limit(0).unwrap();
        assert!(store.begin().is_none());
        store.set_limit(10).unwrap();
        assert!(
            !store
                .append(pending, entry(HistoryStatus::Cancelled))
                .unwrap()
        );
        assert!(
            HistoryStore::load(path, 10)
                .unwrap()
                .get()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn clearing_does_not_allow_pending_requests_to_restore_old_history() {
        let directory = tempfile::tempdir().unwrap();
        let store = HistoryStore::load(directory.path().join("history.json"), 10).unwrap();
        let pending = store.begin().unwrap();
        store.clear().unwrap();
        assert!(
            !store
                .append(pending, entry(HistoryStatus::Success))
                .unwrap()
        );
        store
            .append(store.begin().unwrap(), entry(HistoryStatus::Success))
            .unwrap();
        assert_eq!(store.get().unwrap().len(), 1);
    }

    #[test]
    fn failed_write_preserves_current_records() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let store = HistoryStore::load(path.clone(), 10).unwrap();
        store
            .append(store.begin().unwrap(), entry(HistoryStatus::Success))
            .unwrap();
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(store.clear().is_err());
        assert!(
            store
                .append(store.begin().unwrap(), entry(HistoryStatus::Success))
                .is_err()
        );
        assert_eq!(store.get().unwrap().len(), 1);
    }

    #[test]
    fn startup_enforces_reduced_limit_on_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let store = HistoryStore::load(path.clone(), 3).unwrap();
        for _ in 0..3 {
            store
                .append(store.begin().unwrap(), entry(HistoryStatus::Success))
                .unwrap();
        }
        let loaded = HistoryStore::load(path.clone(), 1).unwrap();
        assert_eq!(loaded.get().unwrap()[0].id, 3);
        assert_eq!(
            HistoryStore::load(path, 100).unwrap().get().unwrap().len(),
            1
        );
    }

    #[test]
    fn damaged_history_is_preserved_and_can_be_reset_without_blocking_startup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        fs::write(&path, "damaged data").unwrap();
        let store = HistoryStore::load(path.clone(), 100).unwrap();
        assert!(store.get().is_err());
        assert!(store.begin().is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), "damaged data");
        store.clear().unwrap();
        assert!(store.get().unwrap().is_empty());
        assert!(store.begin().is_some());
    }
}
