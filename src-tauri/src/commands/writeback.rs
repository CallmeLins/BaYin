//! Sidecar 回写命令（A8 显式入口）。
//!
//! 把封面 `.jpg` / 歌词 `.lrc` 回写到歌曲同目录侧车文件，写后回读校验、失败回滚。
//! 仅对可写源（WebDAV）有意义；只读源返回 `Unsupported`。

use tauri::State;

use crate::db::DbState;
use crate::models::StreamServerConfig;
use crate::source::writeback::{SidecarWriteResult, SidecarWriter};

/// 从 DB 读一个流媒体服务器配置。
fn load_config(db: &State<'_, DbState>, server_id: &str) -> Result<StreamServerConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    match crate::db::servers::get_stream_server_by_id(&conn, server_id) {
        Ok(Some(server)) => Ok(StreamServerConfig::from(&server)),
        Ok(None) => Err(format!("服务器不存在: {server_id}")),
        Err(e) => Err(e.to_string()),
    }
}

/// 仅允许对 WebDAV 源回写。
fn ensure_webdav(config: &StreamServerConfig) -> Result<(), String> {
    if config.is_webdav() {
        Ok(())
    } else {
        Err("该服务器类型不支持 Sidecar 回写（仅 WebDAV）".to_string())
    }
}

fn result_str(r: SidecarWriteResult) -> String {
    match r {
        SidecarWriteResult::Verified => "verified".to_string(),
        SidecarWriteResult::VerificationFailed { reason } => format!("verification_failed: {reason}"),
        SidecarWriteResult::Unsupported => "unsupported".to_string(),
    }
}

/// 把封面字节回写到 `source_url` 的同名 `.jpg` 侧车文件。
/// `data_b64`: base64 编码的图片字节。
#[tauri::command]
pub async fn webdav_write_cover(
    db: State<'_, DbState>,
    server_id: String,
    source_url: String,
    data_b64: String,
) -> Result<String, String> {
    let config = load_config(&db, &server_id)?;
    ensure_webdav(&config)?;
    let data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &data_b64,
    )
    .map_err(|e| format!("base64 解码失败: {e}"))?;

    let writer = crate::source::writeback::webdav::webdav_writer(config);
    tauri::async_runtime::spawn_blocking(move || result_str(writer.write_cover(&source_url, &data)))
        .await
        .map_err(|e| e.to_string())
}

/// 把 LRC 歌词回写到 `source_url` 的同名 `.lrc` 侧车文件。
#[tauri::command]
pub async fn webdav_write_lyrics(
    db: State<'_, DbState>,
    server_id: String,
    source_url: String,
    lrc: String,
) -> Result<String, String> {
    let config = load_config(&db, &server_id)?;
    ensure_webdav(&config)?;

    let writer = crate::source::writeback::webdav::webdav_writer(config);
    tauri::async_runtime::spawn_blocking(move || result_str(writer.write_lyrics(&source_url, &lrc)))
        .await
        .map_err(|e| e.to_string())
}
