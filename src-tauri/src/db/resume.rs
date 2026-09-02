//! 断点恢复扫描状态（`scan_resume` 表）。
//!
//! 配合 `source/scan/incremental.rs`（revision 指纹）使用。
//! 断点续扫思路：扫描过程定期把「已完成的游标」落库；中断后重启从上次
//! `state` 继续。`state` 是来源自定义的 JSON，由各协议自己解释。

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

/// 断点状态行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResumeState {
    /// 来源标识（server_id 或 local:path）。
    pub source_key: String,
    /// 来源自定义的续扫状态（JSON 字符串）。
    pub state: String,
    /// 已处理文件数（进度用）。
    pub processed: u64,
    /// 已知文件总数（断点续扫时来自上次记录；0 = 未知）。
    pub total_known: u64,
    pub started_at: i64,
    pub updated_at: i64,
}

/// upsert 断点状态。
pub fn save_resume(
    conn: &Connection,
    source_key: &str,
    state: &str,
    processed: u64,
    total_known: u64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO scan_resume (source_key, state, processed, total_known, started_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, strftime('%s','now'), strftime('%s','now'))
         ON CONFLICT(source_key) DO UPDATE SET
             state = excluded.state,
             processed = excluded.processed,
             total_known = excluded.total_known,
             updated_at = strftime('%s','now')",
        params![
            source_key,
            state,
            processed as i64,
            total_known as i64
        ],
    )?;
    Ok(())
}

/// 读取断点状态。不存在返回 None。
pub fn load_resume(conn: &Connection, source_key: &str) -> Result<Option<ScanResumeState>> {
    let mut stmt = conn.prepare(
        "SELECT source_key, state, processed, total_known, started_at, updated_at
         FROM scan_resume WHERE source_key = ?1 LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![source_key], |row| {
            Ok(ScanResumeState {
                source_key: row.get(0)?,
                state: row.get(1)?,
                processed: row.get::<_, i64>(2)? as u64,
                total_known: row.get::<_, i64>(3)? as u64,
                started_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        });

    match row {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 清除某来源断点（扫描正常结束后调用）。
pub fn clear_resume(conn: &Connection, source_key: &str) -> Result<()> {
    conn.execute("DELETE FROM scan_resume WHERE source_key = ?1", [source_key])?;
    Ok(())
}

/// 清除所有断点（用户手动"从头扫描"）。
pub fn clear_all_resume(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM scan_resume", [])?;
    Ok(())
}
