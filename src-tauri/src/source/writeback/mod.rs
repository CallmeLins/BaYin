//! 回写能力（A8）骨架。
//!
//! 目标：把封面 / LRC 歌词**回写**到可写源（而非只读）。写完后做**字节回读校验**
//! （复用 primuse `verifySidecarWrite` 思路），失败即回滚并报错。

pub mod webdav;

/// Sidecar 回写结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarWriteResult {
    /// 写入成功，且回读校验通过。
    Verified,
    /// 写入后回读校验失败（已尽力回滚或保持原文件）。
    VerificationFailed { reason: String },
    /// 该源不支持回写（`SourceCapabilities::SIDECAR_WRITABLE` 缺省）。
    Unsupported,
}

/// 回写行为契约。
///
/// 具体实现通过 `SourceConnector` 的 `upload` / `fetch_bytes` 能力拼装：
/// 写 sidecar → `fetch_range` 回读对比 → 返回 Verified / VerificationFailed。
pub trait SidecarWriter: Send + Sync {
    /// 把封面字节写为 `xxx.jpg` 到目标路径旁。
    fn write_cover(&self, source_url: &str, data: &[u8]) -> SidecarWriteResult;
    /// 把 LRC 歌词写为 `xxx.lrc` 到目标路径旁。
    fn write_lyrics(&self, source_url: &str, lrc: &str) -> SidecarWriteResult;
}
