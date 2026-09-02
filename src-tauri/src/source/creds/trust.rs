//! 受信任 TLS/HTTP 主机白名单（A7）。
//!
//! 用于连接自建 NAS：只对用户显式勾选的 `host` 放行非标准 TLS 或明文 HTTP，
//! **绝不全局**放开不安全网络访问。对应 `trusted_hosts` 表。

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

/// 一个受信任主机条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedHost {
    pub host: String,
    /// 是否放行明文 HTTP（自建 NAS 常见）。
    pub allow_http: bool,
    /// 可选：固定的自签证书 SHA256 指纹（形如 `sha256:<hex>`）。
    pub pinned_cert: Option<String>,
    pub created_at: i64,
}

/// 判断某 URL 的主机是否在受信任名单中。
/// 返回 (is_trusted, allow_http)。
pub fn is_trusted(conn: &Connection, url: &str) -> Result<(bool, bool)> {
    let host = extract_host(url);
    let Some(host) = host else {
        return Ok((false, false));
    };
    let mut stmt = conn.prepare(
        "SELECT allow_http FROM trusted_hosts WHERE host = ?1 LIMIT 1",
    )?;
    let row = stmt.query_row(params![host], |r| r.get::<_, i32>(0));
    match row {
        Ok(allow_http) => Ok((true, allow_http != 0)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok((false, false)),
        Err(e) => Err(e),
    }
}

/// 添加 / 更新一个受信任主机。
pub fn upsert_trusted_host(
    conn: &Connection,
    host: &str,
    allow_http: bool,
    pinned_cert: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO trusted_hosts (host, allow_http, pinned_cert, created_at)
         VALUES (?1, ?2, ?3, strftime('%s','now'))
         ON CONFLICT(host) DO UPDATE SET
             allow_http = excluded.allow_http,
             pinned_cert = excluded.pinned_cert",
        params![
            host,
            if allow_http { 1 } else { 0 },
            pinned_cert,
        ],
    )?;
    Ok(())
}

/// 列出所有受信任主机。
pub fn list_trusted_hosts(conn: &Connection) -> Result<Vec<TrustedHost>> {
    let mut stmt = conn.prepare(
        "SELECT host, allow_http, pinned_cert, created_at FROM trusted_hosts ORDER BY host",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TrustedHost {
                host: row.get(0)?,
                allow_http: row.get::<_, i32>(1)? != 0,
                pinned_cert: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(rows)
}

/// 删除一个受信任主机。
pub fn remove_trusted_host(conn: &Connection, host: &str) -> Result<()> {
    conn.execute("DELETE FROM trusted_hosts WHERE host = ?1", [host])?;
    Ok(())
}

/// 从 URL 中提取 host（含端口）。
fn extract_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}
