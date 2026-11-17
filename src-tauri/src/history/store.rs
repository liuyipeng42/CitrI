use super::db::{
    State, backup_corrupt_database, commit_changes, count_rows, open_database, prune_to_limit,
    read_meta, upsert_entry, write_meta,
};
use crate::{
    constants::clamp_history_limit,
    history::{HistorySnapshot, now_millis},
    models::HistoryEntry,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::AtomicU64,
    },
};

pub struct HistoryStore {
    path: PathBuf,
    pub(super) value: Mutex<State>,
    pub(super) latest_query: Arc<AtomicU64>,
}

pub struct HistoryTicket {
    pub id: u64,
    generation: u64,
}

impl HistoryStore {
    pub(super) fn lock_state(&self) -> Result<MutexGuard<'_, State>, String> {
        self.value.lock().map_err(|_| "历史数据库已损坏".into())
    }

    pub fn load(path: PathBuf, limit: u32) -> Result<Self, String> {
        let limit = clamp_history_limit(limit);
        let initialized = (|| {
            let mut connection = open_database(&path)?;
            let max_id: u64 = connection
                .query_row("SELECT coalesce(max(id), 0) FROM history", [], |row| {
                    row.get::<_, i64>(0).map(|id| id as u64)
                })
                .map_err(|e| e.to_string())?;
            let next_id = read_meta(&connection, "next_id")?.max(max_id + 1);
            if count_rows(&connection)? > limit {
                let transaction = connection.transaction().map_err(|e| e.to_string())?;
                prune_to_limit(&transaction, limit)?;
                commit_changes(transaction, true, next_id)?;
            }
            let revision = read_meta(&connection, "revision")?;
            Ok::<_, String>(State {
                connection: Some(connection),
                limit,
                next_id,
                generation: 0,
                revision,
                load_error: None,
            })
        })();
        let value = match initialized {
            Ok(value) => value,
            Err(error) => State {
                connection: None,
                limit,
                next_id: 1,
                generation: 0,
                revision: 0,
                load_error: Some(error),
            },
        };
        Ok(Self {
            path,
            value: Mutex::new(value),
            latest_query: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn begin(&self) -> Option<HistoryTicket> {
        let mut state = self.value.lock().ok()?;
        if state.limit == 0 || state.load_error.is_some() {
            return None;
        }
        let ticket = HistoryTicket {
            id: state.next_id,
            generation: state.generation,
        };
        state.next_id += 1;
        Some(ticket)
    }

    pub fn append(&self, ticket: HistoryTicket, mut entry: HistoryEntry) -> Result<bool, String> {
        let mut state = self.lock_state()?;
        if state.limit == 0 || state.generation != ticket.generation {
            return Ok(false);
        }
        entry.id = ticket.id;
        if entry.sync_id.is_empty() {
            return Err("历史记录标识不能为空".into());
        }
        let (limit, next_id) = (state.limit, state.next_id);
        let connection = state.connection.as_mut().ok_or("历史数据库不可用")?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        upsert_entry(&transaction, &entry)?;
        prune_to_limit(&transaction, limit)?;
        commit_changes(transaction, true, next_id)?;
        state.revision += 1;
        Ok(true)
    }

    pub fn replace(&self, mut snapshot: HistorySnapshot, limit: u32) -> Result<(), String> {
        let mut state = self.lock_state()?;
        let mut next_id = state.next_id;
        let connection = state.connection.as_mut().ok_or("历史数据库不可用")?;
        let existing: HashMap<String, u64> = connection
            .prepare_cached("SELECT sync_id,id FROM history")
            .map_err(|e| e.to_string())?
            .query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64)))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        let previous_clear = read_meta(connection, "cleared_at")?;
        for entry in &mut snapshot.entries {
            entry.id = existing.get(&entry.sync_id).copied().unwrap_or_else(|| {
                let id = next_id;
                next_id += 1;
                id
            });
        }
        let limit = clamp_history_limit(limit);
        snapshot
            .entries
            .sort_by_key(|entry| std::cmp::Reverse((entry.created_at, entry.id)));
        snapshot.entries.truncate(limit);
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let ids: Vec<_> = snapshot.entries.iter().map(|entry| &entry.sync_id).collect();
        let mut changed = transaction
            .execute(
                "DELETE FROM history WHERE sync_id NOT IN (SELECT value FROM json_each(?1))",
                [serde_json::to_string(&ids).map_err(|e| e.to_string())?],
            )
            .map_err(|e| e.to_string())?
            > 0;
        for entry in &snapshot.entries {
            changed |= upsert_entry(&transaction, entry)? > 0;
        }
        changed |= previous_clear != snapshot.cleared_at;
        write_meta(&transaction, "cleared_at", snapshot.cleared_at)?;
        commit_changes(transaction, changed, next_id)?;
        if snapshot.cleared_at > previous_clear {
            state.generation += 1;
        }
        state.next_id = next_id;
        state.limit = limit;
        state.load_error = None;
        if changed {
            state.revision += 1;
        }
        Ok(())
    }

    pub fn set_limit(&self, limit: u32) -> Result<(), String> {
        let limit = clamp_history_limit(limit);
        let mut state = self.lock_state()?;
        if state.limit == limit {
            return Ok(());
        }
        if limit == 0 {
            drop(state);
            self.clear()?;
            state = self.lock_state()?;
        } else {
            let next_id = state.next_id;
            if let Some(connection) = state.connection.as_mut() {
                let transaction = connection.transaction().map_err(|e| e.to_string())?;
                let changed = prune_to_limit(&transaction, limit)? > 0;
                commit_changes(transaction, changed, next_id)?;
                if changed {
                    state.revision += 1;
                }
            }
        }
        state.limit = limit;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut state = self.lock_state()?;
        if state.connection.is_none() {
            backup_corrupt_database(&self.path)?;
            state.connection = Some(open_database(&self.path)?);
        }
        let next_id = state.next_id;
        let connection = state.connection.as_mut().unwrap();
        let cleared_at = now_millis().max(read_meta(connection, "cleared_at")?);
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction
            .execute("DELETE FROM history", [])
            .map_err(|e| e.to_string())?;
        write_meta(&transaction, "cleared_at", cleared_at)?;
        commit_changes(transaction, true, next_id)?;
        let _ =
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA incremental_vacuum;");
        state.load_error = None;
        state.generation += 1;
        state.revision += 1;
        Ok(())
    }

    #[cfg(test)]
    pub fn make_read_only(&self) {
        self.value
            .lock()
            .unwrap()
            .connection
            .as_ref()
            .unwrap()
            .execute_batch("PRAGMA query_only=ON")
            .unwrap();
    }
}
