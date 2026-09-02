//! `stream_servers` 明文凭据 → OS keyring 迁移（A6 引擎）。
//!
//! 设计说明：迁移逻辑与「上层如何消费凭据」解耦，独立成库层函数，便于单测与显式调用。
//! 迁移只做三件事：
//! 1. 对每个 `stream_servers` 行，把非空的 `password` / `access_token` 写入
//!    活动凭据后端（`CredentialStore`，keyring 或文件降级）；
//! 2. 把后端引用 key 写入 `credential_ref` / `oauth_ref` 列；
//! 3. 清空明文 `password` / `access_token` 列（避免二次迁移 & 降低落盘明文暴露）。
//!
//! 该函数是**幂等**的：明文已被清空的行会跳过；重复调用不报错。
//!
//! 注意：真正让播放/认证路径走 keyring 读取，需把 `StreamServerConfig` 消费点
//! （subsonic/jellyfin/webdav 构建请求处）改为先从后端取回——那属于高风险改造，
//! 需 app 运行时验证，不在本引擎内擅自改动热路径。

use rusqlite::{Connection, Result};

use crate::source::creds::{credential_ref_key, CredentialStore};

/// 一次性迁移结果汇总。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialMigrationSummary {
    pub servers_scanned: usize,
    /// 迁移了多少个明文字段（password 或 access_token 计各 1）。
    pub secrets_migrated: usize,
    /// 迁移失败的服务 id 与原因。
    pub failures: Vec<(String, String)>,
}

/// 对全部 `stream_servers` 执行幂等明文凭据迁移。
///
/// 返回摘要。失败不中止整体（逐行容错），并把失败记入 `failures`。
pub fn migrate_stream_server_credentials(
    conn: &Connection,
    store: &dyn CredentialStore,
) -> Result<CredentialMigrationSummary> {
    let mut summary = CredentialMigrationSummary {
        servers_scanned: 0,
        secrets_migrated: 0,
        failures: Vec::new(),
    };

    // 读出所有行（含明文与引用列）。
    let mut stmt = conn.prepare(
        "SELECT id, password, access_token, credential_ref, oauth_ref FROM stream_servers",
    )?;
    let rows: Vec<(String, Option<String>, Option<String>, Option<String>, Option<String>)> =
        stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

    for (server_id, password, access_token, credential_ref, oauth_ref) in rows {
        summary.servers_scanned += 1;

        // password 未迁移且明文非空 → 迁。
        let pw_ref_key = credential_ref_key(&server_id, "password");
        if credential_ref.as_deref() != Some(pw_ref_key.as_str()) {
            if let Some(secret) = password.as_deref().filter(|s| !s.is_empty()) {
                match store.set("bayin", &pw_ref_key, secret) {
                    Ok(()) => {
                        update_ref(conn, &server_id, "credential_ref", &pw_ref_key)?;
                        clear_plain(conn, &server_id, "password")?;
                        summary.secrets_migrated += 1;
                    }
                    Err(e) => summary.failures.push((server_id.clone(), e)),
                }
            }
        }

        // access_token 未迁移且明文非空 → 迁。
        let oa_ref_key = credential_ref_key(&server_id, "access_token");
        if oauth_ref.as_deref() != Some(oa_ref_key.as_str()) {
            if let Some(secret) = access_token.as_deref().filter(|s| !s.is_empty()) {
                match store.set("bayin", &oa_ref_key, secret) {
                    Ok(()) => {
                        update_ref(conn, &server_id, "oauth_ref", &oa_ref_key)?;
                        clear_plain(conn, &server_id, "access_token")?;
                        summary.secrets_migrated += 1;
                    }
                    Err(e) => summary.failures.push((server_id.clone(), e)),
                }
            }
        }
    }

    Ok(summary)
}

fn update_ref(conn: &Connection, server_id: &str, column: &str, key: &str) -> Result<()> {
    let sql = format!("UPDATE stream_servers SET {column} = ?1 WHERE id = ?2");
    conn.execute(&sql, rusqlite::params![key, server_id])?;
    Ok(())
}

fn clear_plain(conn: &Connection, server_id: &str, column: &str) -> Result<()> {
    let sql = format!("UPDATE stream_servers SET {column} = '' WHERE id = ?1");
    conn.execute(&sql, rusqlite::params![server_id])?;
    Ok(())
}

/// 根据某服务的引用 key，从凭据后端读回明文。
/// - 有引用 key 且后端能读到 → Some(secret)。
/// - 无引用 key（尚未迁移）或后端无 → None。
pub fn resolve_secret(
    store: &dyn CredentialStore,
    server_id: &str,
    ref_key: Option<&str>,
) -> Option<String> {
    match ref_key {
        Some(k) => store.get("bayin", k).ok().flatten(),
        None => {
            // 尚未迁移 → 用默认 password 引用 key 尝试（多数服务是 password）。
            let default_key = credential_ref_key(server_id, "password");
            store.get("bayin", &default_key).ok().flatten()
        }
    }
}

/// 将一条服务器的明文与凭据后端引用键合并解析为「可用于连接的明文凭据」（双通道回退）。
///
/// 这是 A6 消费点改造的**库层纯函数**：迁移引擎会清空 DB 明文并写入引用键，
/// 但消费点构造 `StreamServerConfig` 时必须先经这里取回，否则迁移后连接会拿空密码。
///
/// 解析规则：
/// 1. **已迁移**：`credential_ref` / `oauth_ref` 非空 → 优先从 store 取回对应明文；
/// 2. **取回失败**（后端不可用 / 引用键在 store 中缺失）→ **回退到 DB 明文列**，
///    保证老用户或 keyring 异常时连接不回退；
/// 3. **未迁移**：引用键为空 → 直接用 DB 明文列。
///
/// 返回值即最终应填入 `StreamServerConfig` 的 `(password, access_token)`。
/// 纯逻辑、无 DB 连接、无 IO 失败可传播，故完全可离线单测。
pub fn resolve_credential_pair(
    store: &dyn CredentialStore,
    server_id: &str,
    plain_password: &str,
    credential_ref: Option<&str>,
    plain_access_token: Option<String>,
    oauth_ref: Option<&str>,
) -> (String, Option<String>) {
    // password：有引用键优先取回，取回失败/无引用键回退明文。
    let password = match credential_ref {
        Some(k) => match store.get("bayin", k) {
            Ok(Some(v)) => v,
            _ => plain_password.to_string(),
        },
        None => {
            // 未迁移：尝试默认 password 引用 key（某些旧版本迁移可能只写了 credential_ref
            // 而未写全列），取不到再用明文。
            let default_key = credential_ref_key(server_id, "password");
            store
                .get("bayin", &default_key)
                .ok()
                .flatten()
                .unwrap_or_else(|| plain_password.to_string())
        }
    };

    // access_token：有引用键优先取回；否则保留明文（可能为 None）。
    let access_token = match oauth_ref {
        Some(k) => match store.get("bayin", k) {
            Ok(Some(v)) => Some(v),
            _ => plain_access_token,
        },
        None => plain_access_token,
    };

    (password, access_token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::source::creds::FileCredentialStore;
    use rusqlite::Connection;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_store() -> FileCredentialStore {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("bayin-credmig-test-{}-{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        FileCredentialStore::new(&dir)
    }

    /// 内存 DB + 跑满所有 migration（到 v13），建一张带明文的 stream_servers。
    fn db_with_server(password: &str, access_token: Option<&str>) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn.execute(
            "INSERT INTO stream_servers (id, server_type, server_name, server_url, username,
                password, access_token, legacy_auth, enabled, created_at)
             VALUES ('srv-1','subsonic','S','http://x','u', ?1, ?2, 0, 1, 100)",
            rusqlite::params![password, access_token],
        )
        .unwrap();
        conn
    }

    #[test]
    fn migrates_and_blanks_plaintext() {
        let store = tmp_store();
        let conn = db_with_server("s3cret", Some("tok123"));

        let summary = migrate_stream_server_credentials(&conn, &store).unwrap();
        assert_eq!(summary.servers_scanned, 1);
        assert_eq!(summary.secrets_migrated, 2);
        assert!(summary.failures.is_empty());

        // 明文被清空
        let (pw, tok): (String, String) = conn
            .query_row(
                "SELECT password, access_token FROM stream_servers WHERE id='srv-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(pw, "");
        assert_eq!(tok, "");

        // 引用列已写入，且可从后端读回
        let (refk, oak): (String, String) = conn
            .query_row(
                "SELECT credential_ref, oauth_ref FROM stream_servers WHERE id='srv-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(refk.ends_with(":password"));
        assert!(oak.ends_with(":access_token"));
        assert_eq!(store.get("bayin", &refk).unwrap().as_deref(), Some("s3cret"));
        assert_eq!(store.get("bayin", &oak).unwrap().as_deref(), Some("tok123"));
    }

    #[test]
    fn idempotent_second_run_migrates_nothing() {
        let store = tmp_store();
        let conn = db_with_server("s3cret", None);

        let first = migrate_stream_server_credentials(&conn, &store).unwrap();
        assert_eq!(first.secrets_migrated, 1);

        // 第二次调用：明文已空，不再迁移。
        let second = migrate_stream_server_credentials(&conn, &store).unwrap();
        assert_eq!(second.secrets_migrated, 0);
    }

    #[test]
    fn skips_empty_credentials() {
        let store = tmp_store();
        let conn = db_with_server("", None);
        let summary = migrate_stream_server_credentials(&conn, &store).unwrap();
        assert_eq!(summary.secrets_migrated, 0);
    }

    // ---- resolve_credential_pair（双通道回退）----

    #[test]
    fn resolve_uses_keyring_when_migrated() {
        // 已迁移：引用键非空 + store 有值 → 取回 store 值，忽略被清空的明文。
        let store = tmp_store();
        let pw_key = credential_ref_key("srv-1", "password");
        let oa_key = credential_ref_key("srv-1", "access_token");
        store.set("bayin", &pw_key, "k3yringpw").unwrap();
        store.set("bayin", &oa_key, "k3yringtok").unwrap();

        let (pw, tok) = resolve_credential_pair(
            &store, "srv-1",
            "",                  // 迁移后明文被清空
            Some(&pw_key),
            None,
            Some(&oa_key),
        );
        assert_eq!(pw, "k3yringpw");
        assert_eq!(tok.as_deref(), Some("k3yringtok"));
    }

    #[test]
    fn resolve_falls_back_to_plaintext_when_not_migrated() {
        // 未迁移：无引用键 → 直接用明文。
        let store = tmp_store();
        let (pw, tok) = resolve_credential_pair(
            &store, "srv-1",
            "plainpw",
            None,
            Some("plaintok".to_string()),
            None,
        );
        assert_eq!(pw, "plainpw");
        assert_eq!(tok.as_deref(), Some("plaintok"));
    }

    #[test]
    fn resolve_falls_back_when_keyring_missing_value() {
        // 已迁移但 store 中没有该引用键的值（后端被清/换机）→ 回退明文，保证不断连。
        let store = tmp_store();
        let pw_key = credential_ref_key("srv-1", "password");
        let (pw, _tok) = resolve_credential_pair(
            &store, "srv-1",
            "backuppw",
            Some(&pw_key),       // 引用键存在但 store 无此值
            None,
            None,
        );
        assert_eq!(pw, "backuppw");
    }

    #[test]
    fn resolve_keeps_none_access_token_when_not_migrated() {
        let store = tmp_store();
        let (_pw, tok) = resolve_credential_pair(&store, "srv-1", "pw", None, None, None);
        assert_eq!(tok, None);
    }
}
