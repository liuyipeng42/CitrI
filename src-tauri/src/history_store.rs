use crate::{
    history::{HistorySnapshot, now_millis},
    models::{HistoryEntry, HistoryPage, HistorySummary},
};
use rusqlite::{
    Connection, OptionalExtension, Transaction, params, params_from_iter, types::Value,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub struct HistoryStore {
    path: PathBuf,
    value: Mutex<State>,
    latest_query: Arc<AtomicU64>,
}

struct QueryProgress<'a>(&'a Connection);
impl Drop for QueryProgress<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}
struct State {
    connection: Option<Connection>,
    limit: usize,
    next_id: u64,
    generation: u64,
    revision: u64,
    load_error: Option<String>,
}
pub struct HistoryTicket {
    pub id: u64,
    generation: u64,
}

fn database(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let connection = Connection::open(path).map_err(|e| format!("无法打开历史数据库：{e}"))?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    connection.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
        INSERT OR IGNORE INTO metadata VALUES ('next_id',1),('cleared_at',0),('revision',0);
        CREATE TABLE IF NOT EXISTS history (id INTEGER PRIMARY KEY, sync_id TEXT UNIQUE NOT NULL,
            created_at INTEGER NOT NULL, action_name TEXT NOT NULL, preview TEXT NOT NULL,
            status TEXT NOT NULL, payload TEXT NOT NULL, search_text TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS history_time ON history(created_at DESC,id DESC);
        CREATE VIRTUAL TABLE IF NOT EXISTS history_search USING fts5(search_text,content='history',content_rowid='id',tokenize='trigram');
        CREATE TRIGGER IF NOT EXISTS history_insert AFTER INSERT ON history BEGIN
            INSERT INTO history_search(rowid,search_text) VALUES (new.id,new.search_text); END;
        CREATE TRIGGER IF NOT EXISTS history_delete AFTER DELETE ON history BEGIN
            INSERT INTO history_search(history_search,rowid,search_text) VALUES ('delete',old.id,old.search_text); END;
        CREATE TRIGGER IF NOT EXISTS history_update AFTER UPDATE ON history BEGIN
            INSERT INTO history_search(history_search,rowid,search_text) VALUES ('delete',old.id,old.search_text);
            INSERT INTO history_search(rowid,search_text) VALUES (new.id,new.search_text); END;")
        .map_err(|e| format!("无法初始化历史数据库：{e}"))?;
    Ok(connection)
}

fn meta(connection: &Connection, key: &str) -> Result<u64, String> {
    connection
        .query_row("SELECT value FROM metadata WHERE key=?1", [key], |row| {
            row.get::<_, i64>(0).map(|value| value as u64)
        })
        .map_err(|e| e.to_string())
}

fn set_meta(transaction: &Transaction<'_>, key: &str, value: u64) -> Result<(), String> {
    transaction
        .execute(
            "UPDATE metadata SET value=?2 WHERE key=?1",
            params![key, i64::try_from(value).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn put(transaction: &Transaction<'_>, entry: &HistoryEntry) -> Result<usize, String> {
    let payload = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let preview: String = entry.selected_text.chars().take(160).collect();
    let status = serde_json::to_value(entry.status).map_err(|e| e.to_string())?;
    let search = [
        entry.action_name.as_str(),
        entry.model.as_str(),
        &entry.selected_text,
        &entry.prompt,
        &entry.system_prompt,
        &entry.result,
        entry.error.as_deref().unwrap_or(""),
    ]
    .join("\n")
    .to_lowercase();
    transaction.execute("INSERT INTO history(id,sync_id,created_at,action_name,preview,status,payload,search_text)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(sync_id) DO UPDATE SET
        id=excluded.id,created_at=excluded.created_at,action_name=excluded.action_name,preview=excluded.preview,status=excluded.status,
        payload=excluded.payload,search_text=excluded.search_text WHERE history.payload!=excluded.payload",
        params![i64::try_from(entry.id).map_err(|e|e.to_string())?,entry.sync_id,i64::try_from(entry.created_at).map_err(|e|e.to_string())?,entry.action_name,preview,status.as_str().unwrap_or("success"),payload,search])
        .map_err(|e| format!("无法保存历史记录：{e}"))
}

fn prune(transaction: &Transaction<'_>, limit: usize) -> Result<usize, String> {
    transaction.execute("DELETE FROM history WHERE id IN (SELECT id FROM history ORDER BY created_at DESC,id DESC LIMIT -1 OFFSET ?1)", [limit as i64]).map_err(|e| e.to_string())
}

fn commit(transaction: Transaction<'_>, changed: bool, next_id: u64) -> Result<(), String> {
    set_meta(&transaction, "next_id", next_id)?;
    if changed {
        transaction
            .execute("UPDATE metadata SET value=value+1 WHERE key='revision'", [])
            .map_err(|e| e.to_string())?;
    }
    transaction
        .commit()
        .map_err(|e| format!("无法提交历史记录：{e}"))
}

impl State {
    fn connection(&self) -> Result<&Connection, String> {
        if let Some(error) = &self.load_error {
            return Err(error.clone());
        }
        self.connection
            .as_ref()
            .ok_or_else(|| "历史数据库不可用".into())
    }
}

impl HistoryStore {
    pub fn load(path: PathBuf, limit: u32) -> Result<Self, String> {
        let initialized = (|| {
            let mut connection = database(&path)?;
            let next_id = meta(&connection, "next_id")?;
            let count: usize = connection
                .query_row("SELECT count(*) FROM history", [], |row| {
                    row.get::<_, i64>(0).map(|count| count as usize)
                })
                .map_err(|e| e.to_string())?;
            if count > limit.min(1000) as usize {
                let transaction = connection.transaction().map_err(|e| e.to_string())?;
                prune(&transaction, limit.min(1000) as usize)?;
                commit(transaction, true, next_id)?;
            }
            let revision = meta(&connection, "revision")?;
            Ok::<_, String>(State {
                connection: Some(connection),
                limit: limit.min(1000) as usize,
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
                limit: limit.min(1000) as usize,
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
        let mut state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
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
        put(&transaction, &entry)?;
        prune(&transaction, limit)?;
        commit(transaction, true, next_id)?;
        state.revision += 1;
        Ok(true)
    }

    pub fn count(&self) -> Result<usize, String> {
        let state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        state
            .connection()?
            .query_row("SELECT count(*) FROM history", [], |row| {
                row.get::<_, i64>(0).map(|count| count as usize)
            })
            .map_err(|e| e.to_string())
    }

    pub fn revision(&self) -> u64 {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revision
    }

    pub fn entry(&self, id: u64) -> Result<Option<HistoryEntry>, String> {
        let state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        let payload: Option<String> = state
            .connection()?
            .query_row(
                "SELECT payload FROM history WHERE id=?1",
                [i64::try_from(id).map_err(|e| e.to_string())?],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        payload
            .map(|payload| serde_json::from_str(&payload).map_err(|e| e.to_string()))
            .transpose()
    }

    pub fn next_query(&self) -> u64 {
        self.latest_query.fetch_add(1, Ordering::AcqRel) + 1
    }

    #[cfg(test)]
    pub fn page(&self, query: &str, offset: usize, limit: usize) -> Result<HistoryPage, String> {
        self.page_at(query, offset, limit, self.next_query())
    }

    pub fn page_at(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        ticket: u64,
    ) -> Result<HistoryPage, String> {
        let state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        let connection = state.connection()?;
        if self.latest_query.load(Ordering::Acquire) != ticket {
            return Err("搜索请求已更新".into());
        }
        let latest = self.latest_query.clone();
        connection
            .progress_handler(100, Some(move || latest.load(Ordering::Acquire) != ticket))
            .map_err(|e| e.to_string())?;
        let _progress = QueryProgress(connection);
        let total = connection
            .query_row("SELECT count(*) FROM history", [], |row| {
                row.get::<_, i64>(0).map(|count| count as usize)
            })
            .map_err(|e| e.to_string())?;
        let terms: Vec<_> = query
            .trim()
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .take(20)
            .collect();
        let mut clauses = Vec::new();
        let mut values = Vec::<Value>::new();
        let indexed: Vec<_> = terms
            .iter()
            .filter(|term| term.chars().count() >= 3)
            .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
            .collect();
        if !indexed.is_empty() {
            clauses.push(
                "id IN (SELECT rowid FROM history_search WHERE history_search MATCH ?)".to_string(),
            );
            values.push(indexed.join(" AND ").into());
        }
        for term in terms {
            clauses.push("instr(search_text,?)>0".to_string());
            values.push(term.into());
        }
        let filter = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let filtered_total = if filter.is_empty() {
            total
        } else {
            connection
                .query_row(
                    &format!("SELECT count(*) FROM history{filter}"),
                    params_from_iter(values.iter()),
                    |row| row.get::<_, i64>(0).map(|count| count as usize),
                )
                .map_err(|e| e.to_string())?
        };
        values.push((limit.clamp(1, 200) as i64).into());
        values.push((offset.min(1000) as i64).into());
        let mut statement=connection.prepare_cached(&format!("SELECT id,created_at,action_name,preview,status FROM history{filter} ORDER BY created_at DESC,id DESC LIMIT ? OFFSET ?")).map_err(|e|e.to_string())?;
        let entries = statement
            .query_map(params_from_iter(values.iter()), |row| {
                let status: String = row.get(4)?;
                let status =
                    serde_json::from_value(serde_json::Value::String(status)).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                Ok(HistorySummary {
                    id: row.get::<_, i64>(0)? as u64,
                    created_at: row.get::<_, i64>(1)? as u64,
                    action_name: row.get(2)?,
                    preview: row.get(3)?,
                    status,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(HistoryPage {
            entries,
            total,
            filtered_total,
        })
    }

    pub fn snapshot(&self) -> Result<HistorySnapshot, String> {
        let state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        let connection = state.connection()?;
        let bytes: i64 = connection
            .query_row(
                "SELECT coalesce(sum(length(cast(payload AS blob))),0) FROM history",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if bytes > 128 * 1024 * 1024 {
            return Err("历史数据超过 128 MB，请减少保留条数后再备份或同步".into());
        }
        let mut statement = connection
            .prepare_cached("SELECT payload FROM history ORDER BY created_at DESC,id DESC")
            .map_err(|e| e.to_string())?;
        let payloads = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let entries = payloads
            .map(|payload| {
                serde_json::from_str(&payload.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(HistorySnapshot {
            entries,
            cleared_at: meta(connection, "cleared_at")?,
        })
    }

    pub fn replace(&self, mut snapshot: HistorySnapshot, limit: u32) -> Result<(), String> {
        let mut state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        let mut next_id = state.next_id;
        let connection = state.connection.as_mut().ok_or("历史数据库不可用")?;
        let existing: HashMap<String, u64> = connection
            .prepare("SELECT sync_id,id FROM history")
            .map_err(|e| e.to_string())?
            .query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64)))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        let previous_clear = meta(connection, "cleared_at")?;
        for entry in &mut snapshot.entries {
            entry.id = existing.get(&entry.sync_id).copied().unwrap_or_else(|| {
                let id = next_id;
                next_id += 1;
                id
            });
        }
        snapshot
            .entries
            .sort_by_key(|entry| std::cmp::Reverse((entry.created_at, entry.id)));
        snapshot.entries.truncate(limit.min(1000) as usize);
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let ids: Vec<_> = snapshot
            .entries
            .iter()
            .map(|entry| &entry.sync_id)
            .collect();
        let mut changed = transaction
            .execute(
                "DELETE FROM history WHERE sync_id NOT IN (SELECT value FROM json_each(?1))",
                [serde_json::to_string(&ids).map_err(|e| e.to_string())?],
            )
            .map_err(|e| e.to_string())?
            > 0;
        for entry in &snapshot.entries {
            changed |= put(&transaction, entry)? > 0;
        }
        changed |= previous_clear != snapshot.cleared_at;
        set_meta(&transaction, "cleared_at", snapshot.cleared_at)?;
        commit(transaction, changed, next_id)?;
        if snapshot.cleared_at > previous_clear {
            state.generation += 1;
        }
        state.next_id = next_id;
        state.limit = limit.min(1000) as usize;
        state.load_error = None;
        if changed {
            state.revision += 1;
        }
        Ok(())
    }

    pub fn set_limit(&self, limit: u32) -> Result<(), String> {
        let limit = limit.min(1000) as usize;
        let mut state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        if state.limit == limit {
            return Ok(());
        }
        if limit == 0 {
            drop(state);
            self.clear()?;
            state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        } else if let Some(connection) = state.connection.as_mut() {
            let next_id = meta(connection, "next_id")?;
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            let changed = prune(&transaction, limit)? > 0;
            commit(transaction, changed, next_id)?;
            if changed {
                state.revision += 1;
            }
        }
        state.limit = limit;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut state = self.value.lock().map_err(|_| "历史数据库已损坏")?;
        if state.connection.is_none() {
            let backup = self
                .path
                .with_file_name(format!("history-corrupt-{}.db", uuid::Uuid::new_v4()));
            for suffix in ["", "-wal", "-shm"] {
                let mut old = self.path.as_os_str().to_os_string();
                old.push(suffix);
                let mut new = backup.as_os_str().to_os_string();
                new.push(suffix);
                if Path::new(&old).exists() {
                    fs::rename(old, new).map_err(|e| format!("无法备份损坏的数据库：{e}"))?;
                }
            }
            state.connection = Some(database(&self.path)?);
        }
        let next_id = state.next_id;
        let connection = state.connection.as_mut().unwrap();
        let cleared_at = now_millis().max(meta(connection, "cleared_at")?);
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction
            .execute("DELETE FROM history", [])
            .map_err(|e| e.to_string())?;
        set_meta(&transaction, "cleared_at", cleared_at)?;
        commit(transaction, true, next_id)?;
        let _ =
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA incremental_vacuum;");
        state.load_error = None;
        state.generation += 1;
        state.revision += 1;
        Ok(())
    }

    #[cfg(test)]
    pub fn get(&self) -> Result<Vec<HistoryEntry>, String> {
        Ok(self.snapshot()?.entries)
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
