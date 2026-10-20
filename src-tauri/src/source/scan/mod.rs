//! 统一扫描能力子模块。
//!
//! - `incremental.rs`：revision 指纹增量追踪（跨来源统一）。
//! - `checkpoint.rs`：流媒体 BFS 列目录的断点落盘（跨进程恢复）。
//! - 本地扫描的断点状态存取见 `crate::db::resume`（`scan_resume` 表）。

pub mod checkpoint;
pub mod incremental;
