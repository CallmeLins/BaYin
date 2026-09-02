//! 统一分发层：用注册表把"按 server_type 分发"收敛为"按连接器分发"。
//!
//! 这些函数是 `commands/streaming.rs` 里旧 if-else 的替代品。调用方只需：
//! ```rust
//! let reg = crate::source::registry();
//! let conn = connector_for(&reg, &config)?;
//! conn.fetch_all_songs(...)?;
//! ```
//!
//! 为平滑迁移，这里也提供顶层便捷函数 `test_connection / fetch_songs /
//! get_stream_url / get_lyrics`，内部自动用默认注册表解析连接器。

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};

use super::{SourceCapabilities, SourceConnector, SourceRegistry, registry};

/// 从注册表按配置解析连接器。找不到返回带支持清单的错误。
pub fn connector_for<'a>(
    reg: &'a SourceRegistry,
    config: &StreamServerConfig,
) -> Result<&'a dyn SourceConnector, String> {
    reg.resolve(config)
}

// ---------- 顶层便捷函数（默认注册表） ----------

/// 测试连接。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    match registry().resolve(config) {
        Ok(c) => c.test_connection(config),
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

/// 扫描歌曲（全量/增量）。
pub fn fetch_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    existing_modified: Option<&std::collections::HashMap<String, i64>>,
    progress: Option<super::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let reg = registry();
    let conn = reg.resolve(config)?;
    conn.fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
}

/// 生成流 URL。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    match registry().resolve(config) {
        Ok(c) => c.get_stream_url(config, song_id),
        Err(e) => {
            log::error!("get_stream_url 未匹配连接器: {e}");
            String::new()
        }
    }
}

/// 生成播放信息（URL + 认证头）。
pub fn get_stream_play_info(
    config: &StreamServerConfig,
    song_id: &str,
) -> super::StreamPlayInfo {
    match registry().resolve(config) {
        Ok(c) => c.get_stream_play_info(config, song_id),
        Err(e) => {
            log::error!("get_stream_play_info 未匹配连接器: {e}");
            super::StreamPlayInfo {
                url: String::new(),
                headers: None,
            }
        }
    }
}

/// 获取歌词。
pub fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    match registry().resolve(config) {
        Ok(c) => c.get_lyrics(config, song_id),
        Err(_) => None,
    }
}

// ---------- 能力查询辅助 ----------

/// 返回某配置对应连接器的能力位（无法匹配时返回 NONE）。
pub fn capabilities_for(config: &StreamServerConfig) -> SourceCapabilities {
    match registry().resolve(config) {
        Ok(c) => c.capabilities(),
        Err(_) => SourceCapabilities::NONE,
    }
}
