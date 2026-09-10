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

/// 某配置对应连接器是否具备指定能力位。
pub fn supports(config: &StreamServerConfig, capability: u64) -> bool {
    capabilities_for(config).has(capability)
}

/// 能力门禁：不具备时返回带动作名的错误，供命令层统一使用。
pub fn require(
    config: &StreamServerConfig,
    capability: u64,
    action: &str,
) -> Result<(), String> {
    if supports(config, capability) {
        Ok(())
    } else {
        Err(format!("该来源不支持{action}"))
    }
}

// ---------- 文件源操作分发（BROWSE / WRITABLE / CACHEABLE） ----------

/// 列出目录内容。无 `BROWSE` 能力的来源返回 NotSupported。
pub fn list_dir(
    config: &StreamServerConfig,
    dir_url: Option<&str>,
) -> Result<Vec<super::DirEntry>, String> {
    let reg = registry();
    reg.resolve(config)?.list_dir(config, dir_url)
}

/// 读取远端文件字节。
pub fn fetch_bytes(config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
    let reg = registry();
    reg.resolve(config)?.fetch_bytes(config, url)
}

/// 上传本地文件到远端。
pub fn upload(config: &StreamServerConfig, local: &str, remote: &str) -> Result<(), String> {
    let reg = registry();
    reg.resolve(config)?.upload(config, local, remote)
}

/// 删除远端文件 / 目录。
pub fn delete(config: &StreamServerConfig, url: &str) -> Result<(), String> {
    let reg = registry();
    reg.resolve(config)?.delete(config, url)
}

/// 移动 / 重命名远端文件 / 目录。
pub fn move_entry(config: &StreamServerConfig, source: &str, destination: &str) -> Result<(), String> {
    let reg = registry();
    reg.resolve(config)?
        .move_entry(config, source, destination)
}

/// 浏览初始目录。
pub fn root_dir_url(config: &StreamServerConfig) -> Result<String, String> {
    let reg = registry();
    reg.resolve(config)?.root_dir_url(config)
}

/// 扫描指定目录 / 前缀。
pub fn scan_dir(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    existing_modified: Option<&std::collections::HashMap<String, i64>>,
    dir_url: &str,
    progress: Option<super::ScanProgress>,
) -> Result<Vec<ScannedSong>, String> {
    let reg = registry();
    reg.resolve(config)?
        .scan_dir(config, cover_cache, existing_modified, dir_url, progress)
}

/// 探测单首歌元数据（后台补全）。
pub fn probe_metadata(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    song_url: &str,
) -> Option<ScannedSong> {
    match registry().resolve(config) {
        Ok(c) => c.probe_metadata(config, cover_cache, song_url),
        Err(_) => None,
    }
}

/// 目录封面 → data URL。
pub fn folder_cover(
    config: &StreamServerConfig,
    server_id: &str,
    song_url: &str,
    cache_root: Option<&std::path::Path>,
) -> Option<String> {
    match registry().resolve(config) {
        Ok(c) => c.folder_cover(config, server_id, song_url, cache_root),
        Err(_) => None,
    }
}

/// 下载整首歌到本地缓存。
pub fn cache_song(
    config: &StreamServerConfig,
    cache_root: &std::path::Path,
    server_id: &str,
    song_id: &str,
) -> Result<Option<String>, String> {
    let reg = registry();
    reg.resolve(config)?
        .cache_song(config, cache_root, server_id, song_id)
}
