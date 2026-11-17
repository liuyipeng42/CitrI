use super::{
    HistorySnapshot,
    db::{QueryProgress, count_rows, read_meta},
    store::HistoryStore,
};
use crate::models::{HistoryEntry, HistoryPage, HistoryStatus, HistorySummary};
use rusqlite::{OptionalExtension, params_from_iter, types::Value};
use std::sync::atomic::Ordering;

impl HistoryStore {
    pub fn count(&self) -> Result<usize, String> {
        let state = self.lock_state()?;
        count_rows(state.connection()?)
    }

    pub fn revision(&self) -> u64 {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revision
    }

    pub fn entry(&self, id: u64) -> Result<Option<HistoryEntry>, String> {
        let state = self.lock_state()?;
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
        let state = self.lock_state()?;
        let connection = state.connection()?;
        if self.latest_query.load(Ordering::Acquire) != ticket {
            return Err("搜索请求已更新".into());
        }
        let latest = self.latest_query.clone();
        connection
            .progress_handler(100, Some(move || latest.load(Ordering::Acquire) != ticket))
            .map_err(|e| e.to_string())?;
        let _progress = QueryProgress(connection);
        let total = count_rows(connection)?;
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
        let mut statement = connection
            .prepare_cached(&format!(
                "SELECT id,created_at,action_name,preview,status FROM history{filter} ORDER BY created_at DESC,id DESC LIMIT ? OFFSET ?"
            ))
            .map_err(|e| e.to_string())?;
        let entries = statement
            .query_map(params_from_iter(values.iter()), |row| {
                let raw_status = row.get_ref(4)?.as_str()?;
                let status = HistoryStatus::from_db(raw_status).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        format!("未知的历史状态：{raw_status}").into(),
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
        let state = self.lock_state()?;
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
            cleared_at: read_meta(connection, "cleared_at")?,
        })
    }

    #[cfg(test)]
    pub fn get(&self) -> Result<Vec<HistoryEntry>, String> {
        Ok(self.snapshot()?.entries)
    }
}
