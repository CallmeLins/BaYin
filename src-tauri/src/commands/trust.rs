//! 受信任主机命令（A7）。
//!
//! 暴露 `trusted_hosts` 表的增删查，供前端「安全与信任」设置页使用。

use tauri::State;

use crate::db::DbState;
use crate::source::creds::trust::{self, TrustedHost};

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
    host: String,
    allow_http: bool,
    pinned_cert: Option<String>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    trust::upsert_trusted_host(&conn, &host, allow_http, pinned_cert.as_deref())
        .map_err(|e| e.to_string())
}

/// 删除一个受信任主机。
#[tauri::command]
pub fn remove_trusted_host(db: State<'_, DbState>, host: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    trust::remove_trusted_host(&conn, &host).map_err(|e| e.to_string())
}
