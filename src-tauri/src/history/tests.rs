use super::*;
use crate::models::HistoryStatus;
use std::fs;

fn entry(status: HistoryStatus) -> HistoryEntry {
    HistoryEntry {
        sync_id: uuid::Uuid::new_v4().to_string(),
        revived_at: 0,
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
    let path = directory.path().join("history.db");
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
    let path = directory.path().join("history.db");
    let store = HistoryStore::load(path.clone(), 3).unwrap();
    for _ in 0..3 {
        store
            .append(store.begin().unwrap(), entry(HistoryStatus::Success))
            .unwrap();
    }
    let pending = store.begin().unwrap();
    let preserved_in_flight = store.begin().unwrap();
    store.set_limit(2).unwrap();
    store
        .append(preserved_in_flight, entry(HistoryStatus::Success))
        .unwrap();
    assert_eq!(
        HistoryStore::load(path.clone(), 2)
            .unwrap()
            .begin()
            .unwrap()
            .id,
        6
    );
    store.set_limit(1).unwrap();
    assert_eq!(
        HistoryStore::load(path.clone(), 1)
            .unwrap()
            .get()
            .unwrap()
            .len(),
        1
    );
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
    let store = HistoryStore::load(directory.path().join("history.db"), 10).unwrap();
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
fn damaged_history_is_preserved_and_can_be_reset_without_blocking_startup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.db");
    fs::write(&path, "damaged data").unwrap();
    let store = HistoryStore::load(path.clone(), 100).unwrap();
    assert!(store.get().is_err());
    assert!(store.begin().is_none());
    assert_eq!(fs::read_to_string(&path).unwrap(), "damaged data");
    store.clear().unwrap();
    assert!(store.get().unwrap().is_empty());
    assert!(store.begin().is_some());
}

#[test]
fn sqlite_pages_search_unicode_and_load_only_selected_details() {
    let directory = tempfile::tempdir().unwrap();
    let store = HistoryStore::load(directory.path().join("history.db"), 100).unwrap();
    for index in 0..95 {
        let mut row = entry(HistoryStatus::Success);
        row.created_at = index + 1;
        row.selected_text = "ÜBERINPUT 原文".into();
        row.result = "中文回答 \"quoted\" %_".repeat(300);
        store.append(store.begin().unwrap(), row).unwrap();
    }
    let page = store.page("überinput 中文回答", 0, 50).unwrap();
    assert_eq!(page.total, 95);
    assert_eq!(page.filtered_total, 95);
    assert_eq!(page.entries.len(), 50);
    assert_eq!(page.entries[0].id, 95);
    assert_eq!(store.page("", 50, 50).unwrap().entries.len(), 45);
    assert_eq!(store.page("%_", 0, 50).unwrap().filtered_total, 95);
    assert_eq!(store.page("\"quoted\"", 0, 50).unwrap().filtered_total, 95);
    assert_eq!(store.page("不存在", 0, 50).unwrap().filtered_total, 0);
    let stale_ticket = store.next_query();
    let current_ticket = store.next_query();
    assert!(store.page_at("", 0, 50, stale_ticket).is_err());
    assert_eq!(store.page_at("", 0, 50, current_ticket).unwrap().total, 95);
    assert!(store.entry(95).unwrap().unwrap().result.len() > 5000);
    assert!(serde_json::to_vec(&page).unwrap().len() < 20_000);
    let revision = store.revision();
    store.replace(store.snapshot().unwrap(), 100).unwrap();
    assert_eq!(store.revision(), revision);
}
