use super::super::webdav::{DavClient, RemoteRead, UploadResult};
use super::{sample, *};
use crate::models::SyncPreferences;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

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
    let (url, server) = mock_server(vec![("304 Not Modified".into(), String::new(), Vec::new())]);
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
