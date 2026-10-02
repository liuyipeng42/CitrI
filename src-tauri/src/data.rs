use crate::{
    history::{HistorySnapshot, merge_history, now_millis},
    models::{Settings, SyncPreferences, SyncStatus},
};
use atomic_write_file::AtomicWriteFile;
use futures_util::StreamExt;
use reqwest::{Client, Method, StatusCode, Url, header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_HISTORY_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone)]
pub struct DataSnapshot {
    pub settings: Settings,
    pub history: HistorySnapshot,
    pub settings_modified_at: u64,
    pub includes_api_keys: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    application: String,
    format_version: u32,
    app_version: String,
    exported_at: u64,
    settings_modified_at: u64,
    includes_api_keys: bool,
}

pub fn portable_settings(settings: &Settings) -> Settings {
    let mut settings = settings.clone();
    let defaults = Settings::default();
    settings.action_window_width = defaults.action_window_width;
    settings.action_window_height = defaults.action_window_height;
    settings
}

pub fn settings_hash(settings: &Settings) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&portable_settings(settings)).unwrap_or_default())
    )
}

pub fn preserve_api_keys(imported: &mut Settings, current: &Settings) {
    for profile in &mut imported.api_profiles {
        if let Some(local) = current
            .api_profiles
            .iter()
            .find(|local| local.id == profile.id && local.api_url == profile.api_url)
        {
            profile.api_key.clone_from(&local.api_key);
        }
    }
}

struct LimitedWriter<'a, W> {
    writer: &'a mut W,
    remaining: usize,
}
impl<W: Write> Write for LimitedWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::other("数据超过备份大小上限"));
        }
        let written = self.writer.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

fn zip_json<T: Serialize>(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    value: &T,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    writer.start_file(
        name,
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(1))
            .unix_permissions(0o600),
    )?;
    let limited = LimitedWriter {
        writer,
        remaining: limit,
    };
    let mut buffered = BufWriter::with_capacity(64 * 1024, limited);
    serde_json::to_writer(&mut buffered, value)?;
    buffered.flush()?;
    Ok(())
}

pub fn encode_archive(snapshot: &DataSnapshot, include_api_keys: bool) -> Result<Vec<u8>, String> {
    let mut settings = portable_settings(&snapshot.settings);
    if !include_api_keys {
        for profile in &mut settings.api_profiles {
            profile.api_key.clear();
        }
    }
    let manifest = Manifest {
        application: "CitrI".into(),
        format_version: 1,
        app_version: env!("CARGO_PKG_VERSION").into(),
        exported_at: now_millis(),
        settings_modified_at: snapshot.settings_modified_at,
        includes_api_keys: include_api_keys,
    };
    let write = || -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        zip_json(&mut writer, "manifest.json", &manifest, 16 * 1024)?;
        zip_json(&mut writer, "settings.json", &settings, 2 * 1024 * 1024)?;
        zip_json(
            &mut writer,
            "history.json",
            &snapshot.history,
            MAX_HISTORY_BYTES,
        )?;
        let bytes = writer.finish()?.into_inner();
        if bytes.len() > MAX_ARCHIVE_BYTES {
            return Err("ZIP 超出 64 MB 上限，请减少保留的历史记录".into());
        }
        Ok(bytes)
    };
    write().map_err(|error| format!("无法生成备份：{error}"))
}

fn read_zip_json<T: serde::de::DeserializeOwned>(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    limit: usize,
) -> Result<T, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|_| format!("备份缺少 {name}"))?;
    if file.size() > limit as u64 {
        return Err(format!("{name} 解压后过大"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("无法读取 {name}：{error}"))?;
    if bytes.len() > limit {
        return Err(format!("{name} 解压后过大"));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("{name} 格式错误：{error}"))
}

pub fn decode_archive(bytes: &[u8]) -> Result<DataSnapshot, String> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("ZIP 文件不能超过 64 MB".into());
    }
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| "不是有效的 ZIP 备份".to_string())?;
    // Read known files in memory, never extract archive-supplied paths to disk.
    let names: HashSet<_> = archive.file_names().map(str::to_string).collect();
    if archive.len() != 3
        || names.len() != 3
        || !["manifest.json", "settings.json", "history.json"]
            .iter()
            .all(|name| names.contains(*name))
    {
        return Err("不是 CitrI 数据备份，或包含重复、未知文件".into());
    }
    let manifest: Manifest = read_zip_json(&mut archive, "manifest.json", 16 * 1024)?;
    if manifest.application != "CitrI" || manifest.format_version != 1 {
        return Err("不支持此备份格式或版本".into());
    }
    if manifest.settings_modified_at > now_millis().saturating_add(7 * 24 * 60 * 60 * 1000) {
        return Err("备份时间异常，请检查设备时间".into());
    }
    let settings: Settings = read_zip_json(&mut archive, "settings.json", 2 * 1024 * 1024)?;
    let history: HistorySnapshot = read_zip_json(&mut archive, "history.json", MAX_HISTORY_BYTES)?;
    let latest_time = now_millis().saturating_add(7 * 24 * 60 * 60 * 1000);
    if history.cleared_at > latest_time {
        return Err("历史清空时间异常，请检查设备时间".into());
    }
    if history.entries.len() > 10_000 {
        return Err("备份中的历史记录过多".into());
    }
    let mut identities = HashSet::new();
    for entry in &history.entries {
        if entry.created_at > latest_time || entry.revived_at > latest_time {
            return Err("历史记录时间异常，请检查设备时间".into());
        }
        if entry.sync_id.is_empty()
            || entry.sync_id.len() > 128
            || !identities.insert(&entry.sync_id)
        {
            return Err("历史记录标识无效".into());
        }
    }
    Ok(DataSnapshot {
        settings: settings.sanitized(),
        history,
        settings_modified_at: manifest.settings_modified_at,
        includes_api_keys: manifest.includes_api_keys,
    })
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let write = || -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = AtomicWriteFile::open(path)?;
        file.write_all(bytes)?;
        file.commit()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    };
    write().map_err(|error| format!("无法写入文件：{error}"))
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct SyncConfig {
    preferences: SyncPreferences,
    settings_hash: String,
    settings_modified_at: u64,
    last_synced_at: u64,
    remote_etag: String,
    synced_settings_hash: String,
    synced_history_revision: u64,
    synced_api_keys: bool,
}

pub struct SyncStore {
    path: PathBuf,
    value: Mutex<SyncConfig>,
    status: Mutex<SyncStatus>,
    pub operation: tokio::sync::Mutex<()>,
    pub wake: tokio::sync::Notify,
}

impl SyncStore {
    pub fn load(path: PathBuf, settings: &Settings, modified_at: u64) -> Self {
        let mut value: SyncConfig = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let hash = settings_hash(settings);
        if value.settings_hash != hash {
            value.settings_hash = hash;
            value.settings_modified_at = modified_at;
        }
        let status = SyncStatus {
            last_synced_at: value.last_synced_at,
            ..SyncStatus::default()
        };
        Self {
            path,
            value: Mutex::new(value),
            status: Mutex::new(status),
            operation: tokio::sync::Mutex::new(()),
            wake: tokio::sync::Notify::new(),
        }
    }

    pub fn preferences(&self) -> SyncPreferences {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .preferences
            .clone()
    }
    pub fn status(&self) -> SyncStatus {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        status.last_synced_at = self
            .value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at;
        status
    }
    pub fn modified_at(&self) -> u64 {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .settings_modified_at
    }
    pub fn first_sync(&self) -> bool {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at
            == 0
    }

    fn persist(&self, value: &SyncConfig) -> Result<(), String> {
        write_atomic(
            &self.path,
            &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
        )
    }

    pub fn save_preferences(
        &self,
        mut preferences: SyncPreferences,
    ) -> Result<SyncPreferences, String> {
        preferences.url = preferences.url.trim().to_string();
        preferences.username = preferences.username.trim().to_string();
        preferences.interval_minutes = preferences.interval_minutes.clamp(1, 1440);
        if preferences.url.len() > 2000
            || preferences.username.len() > 1000
            || preferences.password.len() > 8000
        {
            return Err("WebDAV 配置过长".into());
        }
        if !preferences.url.is_empty() {
            dav_directory_url(&preferences.url)?;
        }
        if preferences.auto_sync && preferences.url.is_empty() {
            return Err("请先填写 WebDAV 目录地址".into());
        }
        let mut current = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        let mut next = current.clone();
        // A different server is a new synchronization relationship.
        if next.preferences.url != preferences.url
            || next.preferences.username != preferences.username
        {
            next.last_synced_at = 0;
            next.remote_etag.clear();
        }
        next.preferences = preferences.clone();
        self.persist(&next)?;
        *current = next;
        self.wake.notify_one();
        Ok(preferences)
    }

    pub fn note_settings(&self, settings: &Settings) -> Result<(), String> {
        let hash = settings_hash(settings);
        let mut current = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        if current.settings_hash == hash {
            return Ok(());
        }
        let mut next = current.clone();
        next.settings_hash = hash;
        next.settings_modified_at =
            now_millis().max(current.settings_modified_at.saturating_add(1));
        self.persist(&next)?;
        *current = next;
        Ok(())
    }

    pub fn mark_applied(
        &self,
        settings: &Settings,
        modified_at: u64,
        synced: bool,
    ) -> Result<(), String> {
        let mut current = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        let mut next = current.clone();
        next.settings_hash = settings_hash(settings);
        next.settings_modified_at = modified_at;
        if synced {
            next.last_synced_at = now_millis();
        }
        self.persist(&next)?;
        *current = next;
        Ok(())
    }

    pub fn probe_etag(&self, history_revision: u64) -> Option<String> {
        let value = self.value.lock().unwrap_or_else(|e| e.into_inner());
        (!value.remote_etag.is_empty()
            && value.synced_settings_hash == value.settings_hash
            && value.synced_history_revision == history_revision
            && value.synced_api_keys == value.preferences.include_api_keys)
            .then(|| value.remote_etag.clone())
    }

    pub fn remember_remote(&self, etag: String, history_revision: u64) -> Result<(), String> {
        let mut value = self.value.lock().map_err(|_| "同步配置存储已损坏")?;
        let mut next = value.clone();
        next.remote_etag = etag;
        next.synced_settings_hash = next.settings_hash.clone();
        next.synced_history_revision = history_revision;
        next.synced_api_keys = next.preferences.include_api_keys;
        self.persist(&next)?;
        *value = next;
        Ok(())
    }

    pub fn checked(&self) {
        self.value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at = now_millis();
    }

    pub fn set_status(&self, busy: bool, message: String, error: String) -> SyncStatus {
        let last_synced_at = self
            .value
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_synced_at;
        let status = SyncStatus {
            busy,
            last_synced_at,
            message,
            error,
        };
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = status.clone();
        status
    }
}

pub fn merge_snapshots(
    local: &DataSnapshot,
    remote: &DataSnapshot,
    first_sync: bool,
) -> DataSnapshot {
    let remote_wins = first_sync
        || (remote.settings_modified_at, settings_hash(&remote.settings))
            > (local.settings_modified_at, settings_hash(&local.settings));
    let source = if remote_wins { remote } else { local };
    let mut settings = source.settings.clone();
    if !source.includes_api_keys {
        preserve_api_keys(&mut settings, &local.settings);
    }
    let history = merge_history(&local.history, &remote.history, settings.history_limit);
    DataSnapshot {
        settings,
        history,
        settings_modified_at: source.settings_modified_at,
        includes_api_keys: source.includes_api_keys,
    }
}

pub fn same_snapshot(left: &DataSnapshot, right: &DataSnapshot, include_keys: bool) -> bool {
    if right.includes_api_keys != include_keys
        || left.settings_modified_at != right.settings_modified_at
        || left.history.cleared_at != right.history.cleared_at
        || left.history.entries.len() != right.history.entries.len()
    {
        return false;
    }
    let mut a = portable_settings(&left.settings);
    let mut b = portable_settings(&right.settings);
    if !include_keys {
        for profile in a.api_profiles.iter_mut().chain(b.api_profiles.iter_mut()) {
            profile.api_key.clear();
        }
    }
    if a != b {
        return false;
    }
    let remote: std::collections::HashMap<_, _> = right
        .history
        .entries
        .iter()
        .map(|entry| (&entry.sync_id, entry))
        .collect();
    left.history.entries.iter().all(|a| {
        remote.get(&a.sync_id).is_some_and(|b| {
            a.created_at == b.created_at
                && a.revived_at == b.revived_at
                && a.action_name == b.action_name
                && a.model == b.model
                && a.selected_text == b.selected_text
                && a.prompt == b.prompt
                && a.system_prompt == b.system_prompt
                && a.result == b.result
                && a.status == b.status
                && a.error == b.error
        })
    })
}

pub fn dav_directory_url(raw: &str) -> Result<Url, String> {
    let mut url = Url::parse(raw.trim()).map_err(|_| "WebDAV 地址格式不正确".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("请填写 HTTP/HTTPS WebDAV 目录地址，账号密码在下方单独填写".into());
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

pub struct DavClient {
    client: Client,
    directory: Url,
    file: Url,
    preferences: SyncPreferences,
}

pub struct RemoteArchive {
    pub bytes: Vec<u8>,
    pub etag: String,
}

pub enum RemoteRead {
    Unchanged,
    Missing,
    Archive(RemoteArchive),
}
pub enum UploadResult {
    Conflict,
    Written(Option<String>),
}

impl DavClient {
    pub fn new(preferences: SyncPreferences) -> Result<Self, String> {
        let directory = dav_directory_url(&preferences.url)?;
        let file = directory
            .join("citri-sync.zip")
            .map_err(|e| e.to_string())?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            directory,
            file,
            preferences,
        })
    }

    fn request(&self, method: Method, url: Url) -> reqwest::RequestBuilder {
        let request = self.client.request(method, url);
        if self.preferences.username.is_empty() && self.preferences.password.is_empty() {
            request
        } else {
            request.basic_auth(&self.preferences.username, Some(&self.preferences.password))
        }
    }

    fn error(status: StatusCode) -> String {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                "WebDAV 身份验证失败，请检查账号、密码和访问权限".into()
            }
            StatusCode::CONFLICT => "WebDAV 上级目录不存在，请先创建上级目录".into(),
            StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT => {
                "WebDAV 地址发生跳转，请填写跳转后的最终目录地址".into()
            }
            _ => format!("WebDAV 返回 HTTP {status}"),
        }
    }

    pub async fn test(&self) -> Result<(), String> {
        let response = self.request(Method::from_bytes(b"PROPFIND").unwrap(), self.directory.clone()).header("Depth", "0").header(header::CONTENT_TYPE, "application/xml; charset=utf-8").body("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:resourcetype/></d:prop></d:propfind>").send().await.map_err(|e| format!("无法连接 WebDAV：{e}"))?;
        if response.status().is_success() {
            Ok(())
        } else if response.status() == StatusCode::NOT_FOUND {
            Err("WebDAV 目录尚不存在；首次同步将尝试创建此目录，请确保上级目录存在".into())
        } else {
            Err(Self::error(response.status()))
        }
    }

    pub async fn read(&self, etag: Option<&str>) -> Result<RemoteRead, String> {
        let request = self
            .request(Method::GET, self.file.clone())
            .header(header::CACHE_CONTROL, "no-cache");
        let request = if let Some(etag) = etag {
            request.header(header::IF_NONE_MATCH, etag)
        } else {
            request
        };
        let response = request
            .send()
            .await
            .map_err(|e| format!("无法下载同步数据：{e}"))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(RemoteRead::Missing);
        }
        if response.status() == StatusCode::NOT_MODIFIED && etag.is_some() {
            return Ok(RemoteRead::Unchanged);
        }
        if !response.status().is_success() {
            return Err(Self::error(response.status()));
        }
        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.starts_with("W/"))
            .ok_or_else(|| {
                "服务器未返回有效 ETag，无法检测多设备同时修改；请使用支持 ETag 的 WebDAV 服务"
                    .to_string()
            })?
            .to_string();
        if response
            .content_length()
            .is_some_and(|size| size > MAX_ARCHIVE_BYTES as u64)
        {
            return Err("远端同步包超过 64 MB".into());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("下载同步数据失败：{e}"))?;
            if bytes.len().saturating_add(chunk.len()) > MAX_ARCHIVE_BYTES {
                return Err("远端同步包超过 64 MB".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(RemoteRead::Archive(RemoteArchive { bytes, etag }))
    }

    pub async fn write(&self, bytes: Vec<u8>, etag: Option<&str>) -> Result<UploadResult, String> {
        if etag.is_none() {
            let response = self
                .request(
                    Method::from_bytes(b"MKCOL").unwrap(),
                    self.directory.clone(),
                )
                .send()
                .await
                .map_err(|e| format!("无法创建 WebDAV 目录：{e}"))?;
            if !response.status().is_success()
                && response.status() != StatusCode::METHOD_NOT_ALLOWED
            {
                return Err(Self::error(response.status()));
            }
        }
        let request = self
            .request(Method::PUT, self.file.clone())
            .header(header::CONTENT_TYPE, "application/zip")
            .body(bytes);
        let request = if let Some(etag) = etag {
            request.header(header::IF_MATCH, etag)
        } else {
            request.header(header::IF_NONE_MATCH, "*")
        };
        let response = request
            .send()
            .await
            .map_err(|e| format!("无法上传同步数据：{e}"))?;
        if response.status() == StatusCode::PRECONDITION_FAILED {
            return Ok(UploadResult::Conflict);
        }
        if response.status().is_success() {
            Ok(UploadResult::Written(
                response
                    .headers()
                    .get(header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .filter(|etag| !etag.starts_with("W/"))
                    .map(str::to_string),
            ))
        } else {
            Err(Self::error(response.status()))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::{HistoryEntry, HistoryStatus};
    use std::net::TcpListener;

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
            auto_sync: true,
            ..SyncPreferences::default()
        };
        store.save_preferences(preferences.clone()).unwrap();
        store.mark_applied(&settings, 100, true).unwrap();
        assert!(!store.first_sync());
        settings.action_window_width = 900;
        store.note_settings(&settings).unwrap();
        assert_eq!(store.modified_at(), 100);
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

    // Real HTTP requests exercise WebDAV authentication and conditional writes.
    pub(crate) fn mock_server(
        responses: Vec<(String, String, Vec<u8>)>,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/CitrI/", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, headers, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 8192];
                loop {
                    let length = stream.read(&mut buffer).unwrap();
                    assert!(length > 0);
                    bytes.extend_from_slice(&buffer[..length]);
                    if let Some(index) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        let header_text = String::from_utf8_lossy(&bytes[..index]).to_lowercase();
                        let content_length: usize = header_text
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .map(|value| value.trim().parse().unwrap())
                            .unwrap_or(0);
                        if bytes.len() >= index + 4 + content_length {
                            requests.push(header_text);
                            break;
                        }
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
            requests
        });
        (url, thread)
    }

    #[tokio::test]
    async fn webdav_get_put_etag_and_conflict_handling() {
        let archive = encode_archive(&sample(), false).unwrap();
        let (url, server) = mock_server(vec![
            ("207 Multi-Status".into(), String::new(), Vec::new()),
            ("404 Not Found".into(), String::new(), Vec::new()),
            ("201 Created".into(), String::new(), Vec::new()),
            ("201 Created".into(), String::new(), Vec::new()),
            (
                "200 OK".into(),
                "ETag: \"version-one\"\r\n".into(),
                archive.clone(),
            ),
            ("412 Precondition Failed".into(), String::new(), Vec::new()),
        ]);
        let client = DavClient::new(SyncPreferences {
            url,
            username: "tester".into(),
            password: "password".into(),
            ..SyncPreferences::default()
        })
        .unwrap();
        client.test().await.unwrap();
        assert!(matches!(
            client.read(None).await.unwrap(),
            RemoteRead::Missing
        ));
        assert!(matches!(
            client.write(archive.clone(), None).await.unwrap(),
            UploadResult::Written(_)
        ));
        let remote = match client.read(None).await.unwrap() {
            RemoteRead::Archive(remote) => remote,
            _ => panic!("missing archive"),
        };
        assert_eq!(remote.bytes, archive);
        assert_eq!(remote.etag, "\"version-one\"");
        assert!(matches!(
            client
                .write(remote.bytes, Some(&remote.etag))
                .await
                .unwrap(),
            UploadResult::Conflict
        ));
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("propfind /citri/"));
        assert!(requests[0].contains("authorization: basic "));
        assert!(requests[0].contains("depth: 0"));
        assert!(requests[2].starts_with("mkcol /citri/"));
        assert!(requests[3].contains("if-none-match: *"));
        assert!(requests[5].contains("if-match: \"version-one\""));
    }

    #[tokio::test]
    async fn webdav_rejects_authentication_failures_and_missing_etags() {
        let (url, server) = mock_server(vec![
            ("401 Unauthorized".into(), String::new(), Vec::new()),
            ("200 OK".into(), String::new(), b"zip".to_vec()),
        ]);
        let client = DavClient::new(SyncPreferences {
            url,
            ..SyncPreferences::default()
        })
        .unwrap();
        assert!(client.test().await.unwrap_err().contains("身份验证失败"));
        assert!(client.read(None).await.err().unwrap().contains("ETag"));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn unchanged_sync_checks_etag_without_downloading_or_uploading_archives() {
        let (url, server) =
            mock_server(vec![("304 Not Modified".into(), String::new(), Vec::new())]);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sync.json");
        let original = sample();
        let store = SyncStore::load(path.clone(), &original.settings, 100);
        store.mark_applied(&original.settings, 100, true).unwrap();
        store.remember_remote("\"cached\"".into(), 5).unwrap();
        let etag = store.probe_etag(5).unwrap();
        let client = DavClient::new(SyncPreferences {
            url,
            ..SyncPreferences::default()
        })
        .unwrap();
        assert!(matches!(
            client.read(Some(&etag)).await.unwrap(),
            RemoteRead::Unchanged
        ));
        assert!(server.join().unwrap()[0].contains("if-none-match: \"cached\""));
        assert!(store.probe_etag(6).is_none());
        let mut changed = original.settings.clone();
        changed.compact = true;
        store.note_settings(&changed).unwrap();
        assert!(store.probe_etag(5).is_none());
        let loaded = SyncStore::load(path, &changed, store.modified_at());
        assert!(loaded.probe_etag(5).is_none());
        let mut identical = original.clone();
        identical.history.entries[0].id = 99;
        assert!(same_snapshot(&original, &identical, true));
        identical.history.entries[0].result = "changed".into();
        assert!(!same_snapshot(&original, &identical, true));
    }
}
