use crate::models::HistoryEntry;
use rusqlite::{Connection, Transaction, params};
use std::{fs, path::Path, time::Duration};

pub(super) struct QueryProgress<'a>(pub(super) &'a Connection);

impl Drop for QueryProgress<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

pub(super) struct State {
    pub(super) connection: Option<Connection>,
    pub(super) limit: usize,
    pub(super) next_id: u64,
    pub(super) generation: u64,
    pub(super) revision: u64,
    pub(super) load_error: Option<String>,
}

impl State {
    pub(super) fn connection(&self) -> Result<&Connection, String> {
        if let Some(error) = &self.load_error {
            return Err(error.clone());
        }
        self.connection
            .as_ref()
            .ok_or_else(|| "历史数据库不可用".into())
    }
}

pub(super) fn open_database(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let connection = Connection::open(path).map_err(|e| format!("无法打开历史数据库：{e}"))?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
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
            INSERT INTO history_search(rowid,search_text) VALUES (new.id,new.search_text); END;",
        )
        .map_err(|e| format!("无法初始化历史数据库：{e}"))?;
    Ok(connection)
}

pub(super) fn backup_corrupt_database(path: &Path) -> Result<(), String> {
    let backup = path.with_file_name(format!("history-corrupt-{}.db", uuid::Uuid::new_v4()));
    for suffix in ["", "-wal", "-shm"] {
        let mut old = path.as_os_str().to_os_string();
        old.push(suffix);
        let mut new = backup.as_os_str().to_os_string();
        new.push(suffix);
        if Path::new(&old).exists() {
            fs::rename(old, new).map_err(|e| format!("无法备份损坏的数据库：{e}"))?;
        }
    }
    Ok(())
}

pub(super) fn count_rows(connection: &Connection) -> Result<usize, String> {
    connection
        .query_row("SELECT count(*) FROM history", [], |row| {
            row.get::<_, i64>(0).map(|count| count as usize)
        })
        .map_err(|e| e.to_string())
}

pub(super) fn read_meta(connection: &Connection, key: &str) -> Result<u64, String> {
    connection
        .query_row("SELECT value FROM metadata WHERE key=?1", [key], |row| {
            row.get::<_, i64>(0).map(|value| value as u64)
        })
        .map_err(|e| e.to_string())
}

pub(super) fn write_meta(
    transaction: &Transaction<'_>,
    key: &str,
    value: u64,
) -> Result<(), String> {
    transaction
        .execute(
            "UPDATE metadata SET value=?2 WHERE key=?1",
            params![key, i64::try_from(value).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn upsert_entry(
    transaction: &Transaction<'_>,
    entry: &HistoryEntry,
) -> Result<usize, String> {
    let payload = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let preview: String = entry.selected_text.chars().take(160).collect();
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
    let mut statement = transaction
        .prepare_cached(
            "INSERT INTO history(id,sync_id,created_at,action_name,preview,status,payload,search_text)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(sync_id) DO UPDATE SET
        id=excluded.id,created_at=excluded.created_at,action_name=excluded.action_name,preview=excluded.preview,status=excluded.status,
        payload=excluded.payload,search_text=excluded.search_text WHERE history.payload!=excluded.payload",
        )
        .map_err(|e| format!("无法保存历史记录：{e}"))?;
    statement
        .execute(params![
            i64::try_from(entry.id).map_err(|e| e.to_string())?,
            entry.sync_id,
            i64::try_from(entry.created_at).map_err(|e| e.to_string())?,
            entry.action_name,
            preview,
            entry.status.as_str(),
            payload,
            search
        ])
        .map_err(|e| format!("无法保存历史记录：{e}"))
}

pub(super) fn prune_to_limit(transaction: &Transaction<'_>, limit: usize) -> Result<usize, String> {
    transaction
        .execute(
            "DELETE FROM history WHERE id IN (SELECT id FROM history ORDER BY created_at DESC,id DESC LIMIT -1 OFFSET ?1)",
            [limit as i64],
        )
        .map_err(|e| e.to_string())
}

pub(super) fn commit_changes(
    transaction: Transaction<'_>,
    changed: bool,
    next_id: u64,
) -> Result<(), String> {
    write_meta(&transaction, "next_id", next_id)?;
    if changed {
        transaction
            .execute("UPDATE metadata SET value=value+1 WHERE key='revision'", [])
            .map_err(|e| e.to_string())?;
    }
    transaction
        .commit()
        .map_err(|e| format!("无法提交历史记录：{e}"))
}
