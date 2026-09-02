//! 断点恢复扫描命令（A4）。
//!
//! 向前端暴露 `scan_resume` 表的读取/清除能力：
//! - `scan_resume_status`：查询某个来源是否有可续扫的状态；
//! - `scan_resume_clear`：清除某个来源（或全部）的断点状态。

use tauri::State;

use crate::db::{self, DbState, ScanResumeState};

/// 查询某个来源的断点状态。无状态返回 None。
#[tauri::command]
pub fn scan_resume_status(
    db: State<'_, DbState>,
    source_key: String,
) -> Result<Option<ScanResumeState>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::resume::load_resume(&conn, &source_key).map_err(|e| e.to_string())
}

/// 清除某个来源的断点状态（正常扫完后前端主动清理）。
#[tauri::command]
pub fn scan_resume_clear(
    db: State<'_, DbState>,
    source_key: String,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::resume::clear_resume(&conn, &source_key).map_err(|e| e.to_string())
}

/// 清除所有断点状态（用户手动"从头扫描"）。
#[tauri::command]
pub fn scan_resume_clear_all(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::resume::clear_all_resume(&conn).map_err(|e| e.to_string())
}
