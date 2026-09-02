//! 统一扫描能力子模块。
//!
//! - `incremental.rs`：revision 指纹增量追踪（跨来源统一）。
//! - 断点状态存取见 `crate::db::resume`（`scan_resume` 表）。

pub mod incremental;
