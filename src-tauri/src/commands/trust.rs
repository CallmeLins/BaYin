//! 受信任主机命令（A7）。
//!
//! 暴露 `trusted_hosts` 表的增删查，供前端「安全与信任」设置页使用。
//! 同时维护一个**内存快照**（`TrustState`），供 blocking 协议层在请求前
//! 判定连接策略；增删写库成功后同步刷新快照，避免过期。

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::State;

use crate::db::DbState;
use crate::source::creds::trust::{self, HostPolicy, TrustSnapshot, TrustedHost};

/// 受信任主机内存快照的管理状态（A7）。
///
/// blocking 协议层（subsonic/webdav/jellyfin）在发起请求前判定目标 URL 是否
/// 放行时，**拿不到 DB 连接**，因此启动时由 `TrustSnapshot::from_conn` 全量载入
/// 一次快照，供 `policy_for` 做纯内存查询。
///
/// 写库的 `upsert_trusted_host` / `remove_trusted_host` 成功后必须调用
/// `rebuild` 刷新，否则快照会与 DB 不一致（例如刚取消信任的主机仍被放行）。
pub struct TrustState(pub Mutex<TrustSnapshot>);

impl TrustState {
    /// 启动时从已连接的 DB 全量构建快照，并同步进程级快照（供协议层读取）。
    pub fn from_conn(conn: &Connection) -> Result<Self, String> {
        let snap = TrustSnapshot::from_conn(conn).map_err(|e| e.to_string())?;
        trust::sync_current_snapshot(snap.clone());
        Ok(TrustState(Mutex::new(snap)))
    }

    /// 从已连接的 DB 全量重建快照，并同步进程级快照。调用方需在持有 DbState
    /// conn 锁期间调用，与写库同一把锁内完成，避免锁序死锁与「读到旧快照」的窗口。
    pub fn rebuild(&self, conn: &Connection) -> Result<(), String> {
        let snap = TrustSnapshot::from_conn(conn).map_err(|e| e.to_string())?;
        let mut guard = self.0.lock().map_err(|e| e.to_string())?;
        *guard = snap.clone();
        trust::sync_current_snapshot(snap);
        Ok(())
    }
}

/// 列出所有受信任主机。
#[tauri::command]
pub fn list_trusted_hosts(db: State<'_, DbState>) -> Result<Vec<TrustedHost>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    trust::list_trusted_hosts(&conn).map_err(|e| e.to_string())
}

/// 添加/更新一个受信任主机（仅对该主机放行非标准 TLS 或明文 HTTP）。
#[tauri::command]
pub fn upsert_trusted_host(
    db: State<'_, DbState>,
    trust: State<'_, TrustState>,
    host: String,
    allow_http: bool,
    pinned_cert: Option<String>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    trust::upsert_trusted_host(&conn, &host, allow_http, pinned_cert.as_deref())
        .map_err(|e| e.to_string())?;
    // 写库成功后同步刷新内存快照。
    trust.rebuild(&conn)
}

/// 删除一个受信任主机。
#[tauri::command]
pub fn remove_trusted_host(
    db: State<'_, DbState>,
    trust: State<'_, TrustState>,
    host: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    trust::remove_trusted_host(&conn, &host).map_err(|e| e.to_string())?;
    trust.rebuild(&conn)
}

/// 查询某 URL 的受信任连接策略（决策层对外入口，A7）。
///
/// 纯内存查询 `TrustState` 快照，不入库。前端可在设置页输入目标地址，
/// 实时预览该地址将获得的放行策略（trusted / allow_http），也可供内部探测。
#[tauri::command]
pub fn get_host_policy(
    trust: State<'_, TrustState>,
    url: String,
) -> Result<HostPolicy, String> {
    let snap = trust.0.lock().map_err(|e| e.to_string())?;
    Ok(snap.policy_for(&url))
}
