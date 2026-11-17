use super::webdav::dav_directory_url;
use super::*;
use crate::models::{HistoryEntry, HistoryStatus};
use crate::{
    history::HistorySnapshot,
    models::{Settings, SyncPreferences},
};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

mod webdav;
pub(crate) use webdav::mock_server;

pub fn sample() -> DataSnapshot {
    let mut settings = Settings::default();
    settings.api_profiles[0].api_key = "test-api-key".into();
    DataSnapshot {
        settings,
        settings_modified_at: 100,
        includes_api_keys: true,
        history: HistorySnapshot {
            entries: vec![HistoryEntry {
                sync_id: "entry-one".into(),
                revived_at: 0,
                id: 1,
                created_at: 50,
                action_name: "翻译".into(),
                model: "model".into(),
                selected_text: "hello".into(),
                prompt: "翻译 hello".into(),
                system_prompt: "助手".into(),
                result: "你好".into(),
                status: HistoryStatus::Success,
                error: None,
            }],
            cleared_at: 0,
        },
    }
}

#[test]
fn zip_round_trip_and_optional_api_keys() {
    let original = sample();
    let private = decode_archive(&encode_archive(&original, true).unwrap()).unwrap();
    assert_eq!(private.settings.api_profiles[0].api_key, "test-api-key");
    assert_eq!(private.history.entries[0].result, "你好");
    assert_eq!(private.history.entries[0].sync_id, "entry-one");
    let public = decode_archive(&encode_archive(&original, false).unwrap()).unwrap();
    assert!(public.settings.api_profiles[0].api_key.is_empty());
    assert!(!public.includes_api_keys);
    assert_eq!(public.settings_modified_at, 100);
    let bytes = encode_archive(&original, false).unwrap();
    let archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(
        archive.file_names().collect::<HashSet<_>>(),
        HashSet::from(["manifest.json", "settings.json", "history.json"])
    );
}

#[test]
fn rejects_invalid_zip_unknown_paths_and_future_formats() {
    assert!(decode_archive(b"not a zip").is_err());
    let valid = encode_archive(&sample(), false).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(&valid)).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        writer
            .start_file(file.name(), SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&contents).unwrap();
    }
    writer
        .start_file("../../outside.txt", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"unsafe").unwrap();
    assert!(decode_archive(&writer.finish().unwrap().into_inner()).is_err());
    let mut future = sample();
    future.settings_modified_at = u64::MAX;
    assert!(decode_archive(&encode_archive(&future, false).unwrap()).is_err());
    let mut invalid = sample();
    invalid.history.entries[0].sync_id.clear();
    assert!(decode_archive(&encode_archive(&invalid, false).unwrap()).is_err());
    let mut duplicate = sample();
    duplicate
        .history
        .entries
        .push(duplicate.history.entries[0].clone());
    assert!(decode_archive(&encode_archive(&duplicate, false).unwrap()).is_err());
}

#[test]
fn merges_distinct_device_records_despite_local_id_collisions_and_preserves_keys() {
    let local = sample();
    let mut remote = sample();
    remote.settings.font_size = 110;
    remote.settings_modified_at = 200;
    remote.includes_api_keys = false;
    remote.settings.api_profiles[0].api_key.clear();
    remote.history.entries[0].sync_id = "entry-two".into();
    remote.history.entries[0].created_at = 60;
    let merged = merge_snapshots(&local, &remote, false);
    assert_eq!(merged.settings.font_size, 110);
    assert_eq!(merged.settings.api_profiles[0].api_key, "test-api-key");
    assert_eq!(merged.history.entries.len(), 2);
    assert_eq!(
        merge_snapshots(&merged, &remote, false)
            .history
            .entries
            .len(),
        2
    );
    remote.settings.api_profiles[0].api_url = "https://other-provider.invalid".into();
    assert!(
        merge_snapshots(&local, &remote, false)
            .settings
            .api_profiles[0]
            .api_key
            .is_empty()
    );
}

#[test]
fn first_sync_adopts_cloud_settings_and_timestamp_conflicts_converge() {
    let mut local = sample();
    local.settings_modified_at = 300;
    local.settings.font_size = 105;
    let mut remote = sample();
    remote.settings_modified_at = 200;
    remote.settings.font_size = 110;
    assert_eq!(
        merge_snapshots(&local, &remote, true).settings.font_size,
        110
    );
    assert_eq!(
        merge_snapshots(&local, &remote, false).settings.font_size,
        105
    );
    remote.settings_modified_at = 300;
    assert_eq!(
        settings_hash(&merge_snapshots(&local, &remote, false).settings),
        settings_hash(&merge_snapshots(&remote, &local, false).settings)
    );
}

#[test]
fn clear_and_intentional_backup_restore_propagate_without_resurrection() {
    let local = sample();
    let mut cleared = sample();
    cleared.history.entries.clear();
    cleared.history.cleared_at = 70;
    assert!(
        merge_snapshots(&local, &cleared, false)
            .history
            .entries
            .is_empty()
    );
    let mut restored = local.clone();
    restored.history.entries[0].revived_at = 80;
    assert_eq!(
        merge_snapshots(&cleared, &restored, false)
            .history
            .entries
            .len(),
        1
    );
    let mut cleared_again = cleared.clone();
    cleared_again.history.cleared_at = 90;
    assert!(
        merge_snapshots(&cleared_again, &restored, false)
            .history
            .entries
            .is_empty()
    );
}

#[test]
fn sync_configuration_is_local_and_window_sizes_do_not_change_shared_settings() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sync.json");
    let mut settings = sample().settings;
    let store = SyncStore::load(path.clone(), &settings, 100);
    let preferences = SyncPreferences {
        url: "https://dav.example.com/CitrI".into(),
        password: "dav-password".into(),
        ..SyncPreferences::default()
    };
    store.save_preferences(preferences.clone()).unwrap();
    assert!(store.take_pending());
    store.mark_applied(&settings, 100, true).unwrap();
    assert!(!store.first_sync());
    settings.action_window_width = 900;
    store.note_settings(&settings).unwrap();
    assert!(!store.has_pending());
    assert_eq!(store.modified_at(), 100);
    settings.compact = true;
    store.note_settings(&settings).unwrap();
    assert!(store.take_pending());
    assert_eq!(
        SyncStore::load(path, &settings, 100).preferences(),
        preferences
    );
    let mut changed = preferences;
    changed.url = "https://another.example.com/CitrI".into();
    store.save_preferences(changed).unwrap();
    assert!(store.first_sync());
    assert_eq!(store.status().last_synced_at, 0);
    for url in [
        "file:///tmp",
        "https://name:secret@example.com/",
        "https://example.com/?token=secret",
    ] {
        assert!(dav_directory_url(url).is_err());
    }
}
