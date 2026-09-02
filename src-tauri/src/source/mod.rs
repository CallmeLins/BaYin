//! 音乐源连接器抽象层（借鉴 primuse 的 `MusicSourceConnector` 协议）。
//!
//! 目标：把现有 `fetch_stream_songs_internal` 里散落的
//! `is_subsonic()/is_webdav()/is_jellyfin_like()` if-else 分发，收敛为
//! "一个 SourceConnector trait + 一个注册表 + 一个统一分发器"。
//!
//! 之后新增一个音乐源 = 实现一个 `SourceConnector` + 注册到 `SourceRegistry`，
//! 不需要再改命令层、扫描层、播放层。

mod capabilities;
pub mod cache;
pub mod creds;
mod dispatch;
pub mod registry;
pub mod scan;
pub mod writeback;

pub use capabilities::SourceCapabilities;
pub use dispatch::{
    capabilities_for, fetch_songs, get_lyrics, get_stream_play_info, get_stream_url,
    test_connection, connector_for,
};
pub use registry::SourceRegistry;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use std::collections::HashMap;

/// 目录浏览条目（文件型来源通用）。WebDAV 的 `WebDavEntry` 可桥接成它。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub url: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
}

/// 播放信息：URL + 附加请求头。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPlayInfo {
    pub url: String,
    /// 附加到 HTTP 音源的请求头，无认证时为空。
    pub headers: Option<Vec<(String, String)>>,
}

/// 扫描进度回调：`(已处理数, 总数)`。
pub type ScanProgress = std::sync::mpsc::Sender<(usize, usize)>;

/// 音乐源连接器抽象。
///
/// 每个具体来源（Subsonic、WebDAV、Jellyfin、未来的 SMB/云盘等）实现此 trait，
/// 并声明自己的 `capabilities()`。上层通过能力位决定调用哪些方法。
///
/// 设计原则：
/// - 全部方法取 `&self` 并返回 `Result<_, String>`，保持与现有 utils 模块一致；
/// - 非能力方法（如无 `BROWSE` 能力时调用 `list_dir`）返回 `NotSupported` 错误，
///   而不是 panic 或空结果，便于上层提前规避；
/// - 增量扫描/断点恢复作为可选扩展，用 `Option` 参数表达，能力位缺省即忽略。
pub trait SourceConnector: Send + Sync + 'static {
    /// 唯一标识（如 "subsonic"、"webdav"、"jellyfin"），用于注册表查找。
    fn id(&self) -> &'static str;

    /// 是否匹配某 `StreamServerConfig`（按 server_type 判断）。
    fn matches(&self, config: &StreamServerConfig) -> bool;

    /// 本连接器支持的能力位。
    fn capabilities(&self) -> SourceCapabilities;

    // ---------- 连接测试 ----------

    /// 测试连接，返回版本信息等。
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult;

    // ---------- 扫描 ----------

    /// 全量 / 增量扫描库内歌曲。
    ///
    /// `existing_modified`: song_id → 已入库的修改时间，用于增量（能力位
    /// `INCREMENTAL` 才有效）。
    /// `progress`: 扫描进度通道，可选。
    /// `read_tags`: true=逐首探测标签；false=快速扫描。
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String>;

    // ---------- 播放 ----------

    /// 生成歌曲流 URL。
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String;

    /// 生成播放信息（URL + 认证头）。缺省实现：无认证头。
    fn get_stream_play_info(
        &self,
        config: &StreamServerConfig,
        song_id: &str,
    ) -> StreamPlayInfo {
        StreamPlayInfo {
            url: self.get_stream_url(config, song_id),
            headers: None,
        }
    }

    /// 获取歌词。缺省实现：无。
    fn get_lyrics(
        &self,
        config: &StreamServerConfig,
        _song_id: &str,
    ) -> Option<String> {
        let _ = config;
        None
    }

    // ---------- 目录浏览 / 文件操作（BROWSE / WRITABLE 能力） ----------

    /// 列出指定目录内容。无 `BROWSE` 能力时返回 NotSupported。
    fn list_dir(
        &self,
        _config: &StreamServerConfig,
        _dir_url: Option<&str>,
    ) -> Result<Vec<DirEntry>, String> {
        Err("该来源不支持目录浏览".to_string())
    }

    /// 获取远端文件字节（图片等）。无 `BROWSE` 能力时返回 NotSupported。
    fn fetch_bytes(
        &self,
        _config: &StreamServerConfig,
        _url: &str,
    ) -> Result<Vec<u8>, String> {
        Err("该来源不支持字节读取".to_string())
    }

    /// 上传本地文件到远端目录。无 `WRITABLE` 能力时返回 NotSupported。
    fn upload(
        &self,
        _config: &StreamServerConfig,
        _local_path: &str,
        _remote_url: &str,
    ) -> Result<(), String> {
        Err("该来源不支持上传".to_string())
    }

    /// 删除远端文件/目录。无 `WRITABLE` 能力时返回 NotSupported。
    fn delete(
        &self,
        _config: &StreamServerConfig,
        _url: &str,
    ) -> Result<(), String> {
        Err("该来源不支持删除".to_string())
    }

    /// 移动/重命名远端文件/目录。无 `WRITABLE` 能力时返回 NotSupported。
    fn move_entry(
        &self,
        _config: &StreamServerConfig,
        _source: &str,
        _destination: &str,
    ) -> Result<(), String> {
        Err("该来源不支持移动".to_string())
    }
}

// ---------- 为现有 utils 模块提供薄适配器 ----------

/// Subsonic 连接器适配器。
pub struct SubsonicConnector;
/// WebDAV 连接器适配器。
pub struct WebdavConnector;
/// Jellyfin/Emby 连接器适配器。
pub struct JellyfinConnector;

use crate::utils::{jellyfin, subsonic, webdav};

impl SourceConnector for SubsonicConnector {
    fn id(&self) -> &'static str {
        "subsonic"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_subsonic()
    }
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::from_slice(&[
            SourceCapabilities::SCAN_LIBRARY,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::SERVER_PLAYLISTS,
            SourceCapabilities::SERVER_LYRICS,
            SourceCapabilities::READONLY,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        // 阻塞式调用在 async 上下文由调用方 spawn_blocking；这里保持同步。
        pollster::block_on(subsonic::test_connection(config))
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        _existing_modified: Option<&HashMap<String, i64>>,
        _progress: Option<ScanProgress>,
        _read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        pollster::block_on(subsonic::fetch_all_songs(config))
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        subsonic::get_stream_url(config, song_id)
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        pollster::block_on(subsonic::get_lyrics(config, song_id))
    }
}

impl SourceConnector for WebdavConnector {
    fn id(&self) -> &'static str {
        "webdav"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_webdav()
    }
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::from_slice(&[
            SourceCapabilities::BROWSE,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::CACHEABLE,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::WRITABLE,
            SourceCapabilities::SIDECAR_WRITABLE,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        pollster::block_on(webdav::test_connection(config))
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        pollster::block_on(webdav::fetch_all_songs(
            config,
            cover_cache,
            existing_modified,
            progress,
            read_tags,
        ))
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        webdav::stream_url(config, song_id)
    }
    fn get_stream_play_info(
        &self,
        config: &StreamServerConfig,
        song_id: &str,
    ) -> StreamPlayInfo {
        StreamPlayInfo {
            url: webdav::stream_url(config, song_id),
            headers: Some(webdav::stream_headers(config)),
        }
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        // WebDAV 读取同目录 .lrc 侧车文件（阻塞，调用方需 spawn_blocking）
        webdav::get_lyrics(config, song_id)
    }
    fn list_dir(
        &self,
        config: &StreamServerConfig,
        dir_url: Option<&str>,
    ) -> Result<Vec<DirEntry>, String> {
        let entries = webdav::list_dir_entries(config, dir_url)?;
        Ok(entries
            .into_iter()
            .map(|e| DirEntry {
                name: e.name,
                url: e.url,
                is_dir: e.is_directory,
                size: e.size,
                modified: e.modified,
            })
            .collect())
    }
    fn fetch_bytes(&self, config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
        webdav::fetch_bytes(config, url)
    }
    fn upload(&self, config: &StreamServerConfig, local: &str, remote: &str) -> Result<(), String> {
        webdav::upload(config, local, remote)
    }
    fn delete(&self, config: &StreamServerConfig, url: &str) -> Result<(), String> {
        webdav::delete(config, url)
    }
    fn move_entry(
        &self,
        config: &StreamServerConfig,
        source: &str,
        destination: &str,
    ) -> Result<(), String> {
        webdav::move_entry(config, source, destination)
    }
}

impl SourceConnector for JellyfinConnector {
    fn id(&self) -> &'static str {
        "jellyfin"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_jellyfin_like()
    }
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::from_slice(&[
            SourceCapabilities::SCAN_LIBRARY,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::SERVER_PLAYLISTS,
            SourceCapabilities::SERVER_LYRICS,
            SourceCapabilities::READONLY,
            SourceCapabilities::OAUTH, // Jellyfin 用 access_token（Bearer）
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        pollster::block_on(jellyfin::test_connection(config))
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        _existing_modified: Option<&HashMap<String, i64>>,
        _progress: Option<ScanProgress>,
        _read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        pollster::block_on(jellyfin::fetch_all_songs(config))
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        jellyfin::get_stream_url(config, song_id)
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        pollster::block_on(jellyfin::get_lyrics(config, song_id))
    }
}

/// 构建默认注册表（内置三个连接器）。
///
/// 后续新增音乐源时，在此追加注册即可。
pub fn registry() -> SourceRegistry {
    let mut reg = SourceRegistry::new();
    reg.register(Box::new(SubsonicConnector));
    reg.register(Box::new(WebdavConnector));
    reg.register(Box::new(JellyfinConnector));
    reg
}
