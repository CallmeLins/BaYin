//! WebDAV Sidecar 回写实现（A8 具体实现）。
//!
//! 针对可写源（WebDAV）把封面 `.jpg` / 歌词 `.lrc` 侧车文件**回写**到歌曲同目录，
//! 写后**回读字节校验**（复用 primuse `verifySidecarWrite` 思路）；失败即回滚并返回
//! `VerificationFailed`。
//!
//! 回滚策略：
//! - 写入前若目标已存在同名字段，先 `fetch_bytes` 备份旧字节；
//! - 校验失败时：有旧备份 → 写回旧字节；无旧备份 → 删除新写的侧车文件。
//! 尽力回滚，失败也不至于把新文件留成损坏态——而是删除它以恢复「无侧车」原状。

use std::sync::Arc;

use crate::models::StreamServerConfig;
use crate::utils::webdav;

use super::{SidecarWriteResult, SidecarWriter};

/// 校验失败是否允许回滚的保护阈值上限（字节）。超出视为异常，跳过部分回滚保护。
const MAX_SIDECAR_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// 基于 WebDAV 的 Sidecar 回写器。
pub struct WebdavSidecarWriter {
    config: Arc<StreamServerConfig>,
}

impl WebdavSidecarWriter {
    pub fn new(config: StreamServerConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// 把字节写到临时文件，PUT 到 remote，再回读校验；失败则回滚。
    fn write_sidecar(&self, remote_url: &str, data: &[u8], rollback_data: Option<Vec<u8>>) -> SidecarWriteResult {
        // 超长保护
        if data.len() > MAX_SIDECAR_BYTES {
            return SidecarWriteResult::VerificationFailed {
                reason: format!("sidecar 超限 ({} > {MAX_SIDECAR_BYTES} bytes)", data.len()),
            };
        }

        // 写临时文件（webdav::upload 需要本地路径）
        let tmp_path = match write_temp(data) {
            Ok(p) => p,
            Err(e) => {
                return SidecarWriteResult::VerificationFailed {
                    reason: format!("写临时文件失败: {e}"),
                }
            }
        };

        // PUT
        if let Err(e) = webdav::upload(&self.config, &tmp_path, remote_url) {
            let _ = std::fs::remove_file(&tmp_path);
            return SidecarWriteResult::VerificationFailed { reason: e };
        }

        // 回读校验
        let read_back = webdav::fetch_bytes(&self.config, remote_url);
        let _ = std::fs::remove_file(&tmp_path);

        match read_back {
            Ok(bytes) if bytes == data => SidecarWriteResult::Verified,
            Ok(_) => {
                self.rollback(remote_url, rollback_data);
                SidecarWriteResult::VerificationFailed {
                    reason: "回读字节与写入不一致".to_string(),
                }
            }
            Err(e) => {
                self.rollback(remote_url, rollback_data);
                SidecarWriteResult::VerificationFailed {
                    reason: format!("回读失败: {e}"),
                }
            }
        }
    }

    /// 尽力回滚：有旧备份 → 重新上传旧字节；否则删除新侧车。
    fn rollback(&self, remote_url: &str, rollback_data: Option<Vec<u8>>) {
        match rollback_data {
            Some(old) => {
                if let Ok(tmp) = write_temp(&old) {
                    let _ = webdav::upload(&self.config, &tmp, remote_url);
                    let _ = std::fs::remove_file(&tmp);
                }
            }
            None => {
                let _ = webdav::delete(&self.config, remote_url);
            }
        }
    }
}

fn write_temp(data: &[u8]) -> Result<String, String> {
    let path = std::env::temp_dir().join(format!(
        "bayin-sidecar-{}-{:x}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let path_str = path.to_string_lossy().to_string();
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path_str)
}

impl SidecarWriter for WebdavSidecarWriter {
    fn write_cover(&self, source_url: &str, data: &[u8]) -> SidecarWriteResult {
        let cover_url = webdav::sidecar_url(source_url, ".jpg");
        let old = webdav::fetch_bytes(&self.config, &cover_url).ok();
        self.write_sidecar(&cover_url, data, old)
    }

    fn write_lyrics(&self, source_url: &str, lrc: &str) -> SidecarWriteResult {
        let lrc_url = webdav::sidecar_url(source_url, ".lrc");
        let old = webdav::fetch_bytes(&self.config, &lrc_url).ok();
        self.write_sidecar(&lrc_url, lrc.as_bytes(), old)
    }
}

/// 便捷工厂：给某个 WebDAV 配置建回写器（与 SourceCapabilities::SIDECAR_WRITABLE 配合）。
pub fn webdav_writer(config: StreamServerConfig) -> WebdavSidecarWriter {
    WebdavSidecarWriter::new(config)
}
