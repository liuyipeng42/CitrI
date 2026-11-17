use reqwest::Client;
use std::sync::OnceLock;

/// Builds a client once and reuses its connection pool and TLS configuration.
pub fn shared_client(
    cell: &'static OnceLock<Client>,
    build: impl FnOnce() -> reqwest::Result<Client>,
) -> Result<&'static Client, String> {
    if let Some(client) = cell.get() {
        return Ok(client);
    }
    let client = build().map_err(|error| format!("无法初始化网络客户端：{error}"))?;
    Ok(cell.get_or_init(|| client))
}
