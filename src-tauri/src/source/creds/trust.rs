//! 受信任 TLS/HTTP 主机白名单（A7）。
//!
//! 用于连接自建 NAS：只对用户显式勾选的 `host` 放行非标准 TLS 或明文 HTTP，
//! **绝不全局**放开不安全网络访问。对应 `trusted_hosts` 表。
//!
//! 除 DB 层 CRUD 外，还提供**纯逻辑的内存快照 + 连接策略判定**（`TrustSnapshot` /
//! `HostPolicy`），供 blocking 协议层（subsonic/webdav/jellyfin）在每次请求前
//! 按目标 URL 决定是否放行、走 http 还是 https、是否注入固定自签证书。

use std::collections::HashMap;

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

/// 从 URL 提取 host（**不含端口**），小写归一。
///
/// 供 `is_trusted` 的按纯 host 查表用；需要区分端口时用 `url_host_and_port`。
pub fn extract_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
}

/// 从 URL 提取 (host, 显式端口)。端口为 URL 中显式写出的端口；
/// 无显式端口时按 scheme 补默认（http=80 / https=443）。host 小写归一。
pub fn url_host_and_port(url: &str) -> Option<(String, u16)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let port = parsed
        .port()
        .or_else(|| match parsed.scheme() {
            "https" => Some(443),
            "http" => Some(80),
            _ => None,
        })
        .unwrap_or(0);
    Some((host, port))
}

/// 归一化 trusted_hosts 的 host 条目：小写、去空白。
pub fn normalize_host(host: &str) -> String {
    host.trim().to_lowercase()
}

/// 某 URL 应采用的连接策略（由 `TrustSnapshot` 判定，纯逻辑、可单测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPolicy {
    /// URL 的主机是否在受信任名单中。
    pub trusted: bool,
    /// 该主机是否放行明文 HTTP。
    pub allow_http: bool,
}

/// 受信任主机的内存快照。
///
/// 启动时从 `trusted_hosts` 表一次性载入，供 blocking 协议层在请求发起前
/// 判定策略——协议函数拿不到 DB 连接，因此需要一个进程内只读快照。
#[derive(Debug, Default, Clone)]
pub struct TrustSnapshot {
    /// key = `normalize_host(entry)`（纯 host 或 `host:port`）。
    entries: HashMap<String, TrustedHost>,
}

impl TrustSnapshot {
    /// 从 DB 连接一次性构建快照（幂等：全量覆盖重建）。
    pub fn from_conn(conn: &Connection) -> Result<Self> {
        let rows = list_trusted_hosts(conn)?;
        let entries = rows
            .into_iter()
            .map(|h| (normalize_host(&h.host), h))
            .collect();
        Ok(TrustSnapshot { entries })
    }

    /// 判定某 URL 的连接策略。
    ///
    /// 匹配规则（精确、无通配，避免过度信任；host 大小写不敏感）：
    /// - 条目为纯 host（`nas.local` / `192.168.1.5`）→ 匹配 URL 的 host（任意端口）；
    /// - 条目为 `host:port`（`192.168.1.5:8080`）→ 仅匹配 URL 的 host+端口完全一致。
    ///
    /// 判定时对 URL 依次尝试 `host:port` 与纯 `host` 两个候选键。
    pub fn policy_for(&self, url: &str) -> HostPolicy {
        let Some((host, port)) = url_host_and_port(url) else {
            return HostPolicy { trusted: false, allow_http: false };
        };

        // 候选 1：精确 `host:port` 条目。
        let authority = format!("{host}:{port}");
        if let Some(h) = self.entries.get(&authority) {
            return HostPolicy { trusted: true, allow_http: h.allow_http };
        }
        // 候选 2：纯 host 条目（任意端口）。
        if let Some(h) = self.entries.get(&host) {
            return HostPolicy { trusted: true, allow_http: h.allow_http };
        }
        HostPolicy { trusted: false, allow_http: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use rusqlite::Connection;

    /// 内存 DB + 预置若干受信任主机条目。
    fn conn_with_hosts(hosts: &[(&str, bool)]) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        for (host, allow_http) in hosts {
            upsert_trusted_host(&conn, host, *allow_http, None).unwrap();
        }
        conn
    }

    #[test]
    fn extract_host_lowercases_and_strips_port() {
        assert_eq!(extract_host("https://NAS.Local:8443/x").as_deref(), Some("nas.local"));
        assert_eq!(extract_host("http://192.168.1.5:8080").as_deref(), Some("192.168.1.5"));
        assert_eq!(extract_host("not a url"), None);
    }

    #[test]
    fn host_and_port_parses_explicit_and_default() {
        assert_eq!(url_host_and_port("https://nas.local:8443/a"), Some(("nas.local".into(), 8443)));
        assert_eq!(url_host_and_port("http://nas.local"), Some(("nas.local".into(), 80)));
        assert_eq!(url_host_and_port("https://nas.local"), Some(("nas.local".into(), 443)));
        assert_eq!(url_host_and_port("not a url"), None);
        assert_eq!(url_host_and_port("nas.local"), None); // 无 scheme → 解析失败
    }

    #[test]
    fn policy_untrusted_by_default() {
        let snap = TrustSnapshot::from_conn(&conn_with_hosts(&[])).unwrap();
        assert_eq!(
            snap.policy_for("https://random.example.com"),
            HostPolicy { trusted: false, allow_http: false }
        );
    }

    #[test]
    fn policy_pure_host_matches_any_port_case_insensitive() {
        let snap = TrustSnapshot::from_conn(&conn_with_hosts(&[("nas.local", true)])).unwrap();
        // 纯 host 条目 → 任意端口、大小写不敏感都命中，且读取 allow_http。
        assert_eq!(
            snap.policy_for("http://NAS.LOCAL:8080/music"),
            HostPolicy { trusted: true, allow_http: true }
        );
        assert_eq!(
            snap.policy_for("https://nas.local/"),
            HostPolicy { trusted: true, allow_http: true }
        );
    }

    #[test]
    fn policy_host_with_port_only_matches_exact_port() {
        let snap = TrustSnapshot::from_conn(&conn_with_hosts(&[("192.168.1.5:8080", false)])).unwrap();
        assert_eq!(
            snap.policy_for("http://192.168.1.5:8080/x"),
            HostPolicy { trusted: true, allow_http: false }
        );
        // 端口不同 → 不信任。
        assert_eq!(
            snap.policy_for("http://192.168.1.5:8081/x"),
            HostPolicy { trusted: false, allow_http: false }
        );
    }

    #[test]
    fn policy_prefers_exact_host_port_over_pure_host() {
        // 同时有纯 host（放行）与 host:port（不放行），访问该端口应命中更精确条目。
        let conn = conn_with_hosts(&[("nas.local", true), ("nas.local:8443", false)]);
        let snap = TrustSnapshot::from_conn(&conn).unwrap();
        assert_eq!(
            snap.policy_for("https://nas.local:8443/a"),
            HostPolicy { trusted: true, allow_http: false }
        );
        assert_eq!(
            snap.policy_for("https://nas.local:9000/a"),
            HostPolicy { trusted: true, allow_http: true }
        );
    }
}


