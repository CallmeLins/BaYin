//! 凭据迁移命令（A6 显式触发入口）。
//!
//! 让前端可以显式发起「明文 → OS keyring」迁移，并查询迁移状态。
//! 底层复用 `db::creds_migrate::migrate_stream_server_credentials`（幂等）。

use tauri::{AppHandle, Manager, State};

use crate::db::DbState;

/// 迁移结果摘要（前端可见）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialMigrationResult {
    pub servers_scanned: usize,
    pub secrets_migrated: usize,
    pub failures: Vec<String>,
}

/// 触发全量明文凭据迁移。幂等：已迁移的行自动跳过。
#[tauri::command]
pub fn migrate_stream_credentials(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<CredentialMigrationResult, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let store = crate::source::creds::default_store(&app_data_dir);

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let summary = crate::db::creds_migrate::migrate_stream_server_credentials(&conn, store.as_ref())
        .map_err(|e| e.to_string())?;

    Ok(CredentialMigrationResult {
        servers_scanned: summary.servers_scanned,
        secrets_migrated: summary.secrets_migrated,
        failures: summary
            .failures
            .into_iter()
            .map(|(id, e)| format!("{id}: {e}"))
            .collect(),
    })
}

/// 当前凭据后端类型（前端提示用）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialBackendInfo {
    pub backend: String,
    /// 明文列里还剩多少条待迁移（0 = 已全部迁完）。
    pub pending_plaintext_secrets: usize,
}

/// 查询凭据迁移状态（当前后端 + 剩余明文条数）。
#[tauri::command]
pub fn get_credential_migration_status(
    db: State<'_, DbState>,
) -> Result<CredentialBackendInfo, String> {
    // 判断后端类型：feature 编译期决定。
    #[cfg(all(feature = "keyring", not(any(target_os = "android", target_os = "ios"))))]
    let backend = "os_keyring";
    #[cfg(not(all(feature = "keyring", not(any(target_os = "android", target_os = "ios")))))]
    let backend = "file_fallback";

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let pending: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM stream_servers
             WHERE (password IS NOT NULL AND password <> '')
                OR (access_token IS NOT NULL AND access_token <> '')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(CredentialBackendInfo {
        backend: backend.to_string(),
        pending_plaintext_secrets: pending,
    })
}
