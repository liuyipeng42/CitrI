use crate::{
    history::HistoryStore,
    models::{ActionWindowSession, SelectionPayload},
    selection::SelectionController,
    settings::SettingsStore,
};
use std::{
    collections::HashMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tauri::menu::CheckMenuItem;
use tokio_util::sync::CancellationToken;

pub struct RequestHandle {
    pub id: u64,
    pub cancellation: CancellationToken,
}

pub struct AppState {
    pub settings: SettingsStore,
    pub history: HistoryStore,
    pub selection: SelectionController,
    sessions: Mutex<HashMap<String, ActionWindowSession>>,
    requests: Mutex<HashMap<String, RequestHandle>>,
    latest_selection: Mutex<Option<SelectionPayload>>,
    tray_toggle: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    next_action_id: AtomicU64,
    next_request_id: AtomicU64,
    quitting: AtomicBool,
}

impl AppState {
    pub fn new(settings: SettingsStore, history: HistoryStore) -> Self {
        let enabled = settings.read(|settings| settings.enabled);
        Self {
            settings,
            history,
            selection: SelectionController::new(enabled),
            sessions: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
            latest_selection: Mutex::new(None),
            tray_toggle: Mutex::new(None),
            next_action_id: AtomicU64::new(1),
            next_request_id: AtomicU64::new(1),
            quitting: AtomicBool::new(false),
        }
    }

    pub fn insert_session(&self, label: String, session: ActionWindowSession) {
        lock_recover(&self.sessions).insert(label, session);
    }

    pub fn with_session<T>(
        &self,
        label: &str,
        reader: impl FnOnce(&ActionWindowSession) -> T,
    ) -> Option<T> {
        lock_recover(&self.sessions).get(label).map(reader)
    }

    pub fn with_session_mut<T>(
        &self,
        label: &str,
        update: impl FnOnce(&mut ActionWindowSession) -> T,
    ) -> Option<T> {
        lock_recover(&self.sessions).get_mut(label).map(update)
    }

    pub fn remove_session(&self, label: &str) -> Option<ActionWindowSession> {
        lock_recover(&self.sessions).remove(label)
    }

    pub fn next_action_id(&self) -> u64 {
        self.next_action_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn replace_request(&self, label: String, request: RequestHandle) -> Option<RequestHandle> {
        lock_recover(&self.requests).insert(label, request)
    }

    pub fn request_is_current(&self, label: &str, request_id: u64) -> bool {
        lock_recover(&self.requests)
            .get(label)
            .is_some_and(|request| request.id == request_id)
    }

    pub fn finish_request(&self, label: &str, request_id: u64) -> bool {
        let mut requests = lock_recover(&self.requests);
        if requests
            .get(label)
            .is_some_and(|request| request.id == request_id)
        {
            requests.remove(label);
            true
        } else {
            false
        }
    }

    pub fn remove_request(&self, label: &str) -> Option<RequestHandle> {
        lock_recover(&self.requests).remove(label)
    }

    pub fn latest_selection(&self) -> Option<SelectionPayload> {
        lock_recover(&self.latest_selection).clone()
    }

    pub fn set_latest_selection(&self, selection: SelectionPayload) {
        *lock_recover(&self.latest_selection) = Some(selection);
    }

    pub fn tray_toggle(&self) -> Option<CheckMenuItem<tauri::Wry>> {
        lock_recover(&self.tray_toggle).clone()
    }

    pub fn set_tray_toggle(&self, toggle: CheckMenuItem<tauri::Wry>) {
        *lock_recover(&self.tray_toggle) = Some(toggle);
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::Acquire)
    }

    pub fn mark_quitting(&self) {
        self.quitting.store(true, Ordering::Release);
    }
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
