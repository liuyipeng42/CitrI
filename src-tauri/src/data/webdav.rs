use super::archive::MAX_ARCHIVE_BYTES;
use crate::models::SyncPreferences;
use futures_util::StreamExt;
use reqwest::{Client, Method, StatusCode, Url, header};
use std::time::Duration;

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
