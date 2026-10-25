use super::snapshot::{DataSnapshot, portable_settings};
use crate::{
    history::{HistorySnapshot, now_millis},
    models::Settings,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::{BufWriter, Cursor, Read, Write},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_HISTORY_BYTES: usize = 256 * 1024 * 1024;

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
