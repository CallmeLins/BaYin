//! 音乐源连接器抽象层（借鉴 primuse 的 `MusicSourceConnector` 协议）。
//!
//! 目标：把现有 `fetch_stream_songs_internal` 里散落的
//! `is_subsonic()/is_webdav()/is_jellyfin_like()` if-else 分发，收敛为
//! "一个 SourceConnector trait + 一个注册表 + 一个统一分发器"。
//!
//! 之后新增一个音乐源 = 实现一个 `SourceConnector` + 注册到 `SourceRegistry`，
//! 不需要再改命令层、扫描层、播放层。

mod capabilities;
pub mod bridge;
pub mod cache;
pub mod creds;
mod dispatch;
pub mod registry;
pub mod scan;
pub mod throttle;
pub mod writeback;

pub use capabilities::SourceCapabilities;
pub use dispatch::{
    cache_song, capabilities_for, delete, fetch_bytes, fetch_songs, folder_cover, get_lyrics,
    get_stream_play_info, get_stream_url, list_dir, move_entry, probe_metadata, require,
    root_dir_url, scan_dir, supports, test_connection, upload,
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

    // ---------- 文件源扩展操作（BROWSE / WRITABLE / CACHEABLE） ----------
    //
    // 这些方法让"文件型来源"（WebDAV / S3 / 未来的 SMB / FTP 等）在命令层
    // 可以完全按能力位分发，而命令层无需再认识任何具体协议。

    /// 浏览初始目录（BROWSE）。返回值语义由来源自定义：
    /// WebDAV 为完整 URL，S3 为空串（表示桶根 / 配置前缀）。
    fn root_dir_url(&self, _config: &StreamServerConfig) -> Result<String, String> {
        Err("该来源不支持目录浏览".to_string())
    }

    /// 扫描指定目录 / 前缀（BROWSE）。默认 NotSupported。
    fn scan_dir(
        &self,
        _config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        _existing_modified: Option<&HashMap<String, i64>>,
        _dir_url: &str,
        _progress: Option<ScanProgress>,
    ) -> Result<Vec<ScannedSong>, String> {
        Err("该来源不支持目录扫描".to_string())
    }

    /// 探测单首歌标签（快速扫描后的后台元数据补全）。默认 None。
    fn probe_metadata(
        &self,
        _config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        _song_url: &str,
    ) -> Option<ScannedSong> {
        None
    }

    /// 目录封面（cover.jpg / folder.jpg 等）→ data URL。默认 None。
    ///
    /// 提供基于 `list_dir` + `fetch_bytes` 的**通用实现**，任何具备 `BROWSE`
    /// 能力的来源即可获得封面回退；WebDAV 等有持久缓存的来源可覆盖以提升性能。
    fn folder_cover(
        &self,
        config: &StreamServerConfig,
        _server_id: &str,
        song_url: &str,
        _cache_root: Option<&std::path::Path>,
    ) -> Option<String> {
        if !self.capabilities().has(SourceCapabilities::BROWSE) {
            return None;
        }
        let dir = song_url
            .rsplit_once('/')
            .map(|(d, _)| format!("{d}/"))
            .unwrap_or_default();
        let entries = self.list_dir(config, Some(&dir)).ok()?;
        let cover = entries
            .iter()
            .find(|e| !e.is_dir && crate::utils::webdav::is_cover_image_name(&e.name))?;
        let bytes = self.fetch_bytes(config, &cover.url).ok()?;
        Some(crate::utils::webdav::image_data_url(&cover.name, &bytes))
    }

    /// 下载整首歌到本地缓存，返回缓存路径（CACHEABLE）。默认 None（不缓存）。
    fn cache_song(
        &self,
        _config: &StreamServerConfig,
        _cache_root: &std::path::Path,
        _server_id: &str,
        _song_id: &str,
    ) -> Result<Option<String>, String> {
        Ok(None)
    }

    // ---------- 非 HTTP 来源的本地桥接（B2/B4） ----------

    /// 打开一个可随机读取的读取器，供 [`super::bridge`] 在 `127.0.0.1` 上暴露成
    /// 支持 Range 的 HTTP 流（播放引擎只会说 HTTP）。
    ///
    /// 只有**非 HTTP 来源**需要实现它；HTTP 原生来源（WebDAV/S3/UPnP）直接给
    /// 直链即可，无需桥接。默认 NotSupported。
    fn open_reader(
        &self,
        _config: &StreamServerConfig,
        _song_id: &str,
    ) -> Result<Box<dyn bridge::RangeReader>, String> {
        Err("该来源不需要本地桥接".to_string())
    }
}

// ---------- 为现有 utils 模块提供薄适配器 ----------

/// Subsonic 连接器适配器。
pub struct SubsonicConnector;
/// WebDAV 连接器适配器。
pub struct WebdavConnector;
/// Jellyfin/Emby 连接器适配器。
pub struct JellyfinConnector;
/// S3 / S3 兼容对象存储连接器（阶段 B5）。
pub struct S3Connector;
/// UPnP / DLNA 媒体服务器连接器（阶段 B6）。
pub struct UpnpConnector;
/// FTP 连接器（阶段 B4；经本地 HTTP+Range 桥接播放）。
pub struct FtpConnector;
/// 本地 / 已挂载路径连接器（阶段 B2/B3：SMB-UNC、CIFS/NFS 挂载点、本地目录）。
pub struct LocalConnector;
/// Plex 媒体服务器连接器（阶段 C1）。
pub struct PlexConnector;
/// 飞牛音乐连接器（阶段 C2；`authx` 动态签名，经本地桥接播放）。
pub struct FnMusicConnector;
/// 道理鱼连接器（阶段 C3；Bearer 认证，经本地桥接播放）。
pub struct DaoLiYuConnector;
/// 115 网盘连接器（阶段 D1；Bearer + 轮换 refresh_token，HTTP 原生直链）。
pub struct Pan115Connector;

use crate::utils::{
    daoliyu, fnmusic, ftp, jellyfin, localfs, pan115, plex, s3, subsonic, upnp, webdav,
};

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
    fn root_dir_url(&self, config: &StreamServerConfig) -> Result<String, String> {
        let base = webdav::clean_base_url(config);
        let path = webdav::effective_initial_path_public(config);
        Ok(format!("{base}{path}"))
    }
    fn scan_dir(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        dir_url: &str,
        progress: Option<ScanProgress>,
    ) -> Result<Vec<ScannedSong>, String> {
        webdav::scan_dir(config, cover_cache, existing_modified, dir_url, progress)
    }
    fn probe_metadata(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        song_url: &str,
    ) -> Option<ScannedSong> {
        webdav::probe_song(config, cover_cache, song_url)
    }
    fn folder_cover(
        &self,
        config: &StreamServerConfig,
        server_id: &str,
        song_url: &str,
        cache_root: Option<&std::path::Path>,
    ) -> Option<String> {
        // WebDAV 覆盖默认实现：带进程内 + 磁盘持久缓存。
        webdav::folder_cover(config, server_id, song_url, cache_root)
    }
    fn cache_song(
        &self,
        config: &StreamServerConfig,
        cache_root: &std::path::Path,
        server_id: &str,
        song_id: &str,
    ) -> Result<Option<String>, String> {
        webdav::cache_song(config, cache_root, server_id, song_id).map(Some)
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

impl SourceConnector for S3Connector {
    fn id(&self) -> &'static str {
        "s3"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_s3()
    }
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::from_slice(&[
            SourceCapabilities::BROWSE,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::CACHEABLE,
            SourceCapabilities::READONLY,
            SourceCapabilities::TRUSTED_HOST,
            SourceCapabilities::SERVER_LYRICS,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        s3::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        s3::fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        match s3::S3Config::from_stream_config(config).and_then(|c| s3::presign_get_url(&c, song_id))
        {
            Ok(url) => url,
            Err(e) => {
                log::error!("S3 生成播放 URL 失败: {e}");
                String::new()
            }
        }
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        s3::get_lyrics(config, song_id)
    }
    fn list_dir(
        &self,
        config: &StreamServerConfig,
        dir_url: Option<&str>,
    ) -> Result<Vec<DirEntry>, String> {
        let cfg = s3::S3Config::from_stream_config(config)?;
        s3::list_dir(&cfg, dir_url.unwrap_or(""))
    }
    fn fetch_bytes(&self, config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
        let cfg = s3::S3Config::from_stream_config(config)?;
        s3::fetch_bytes(&cfg, url)
    }
    fn root_dir_url(&self, _config: &StreamServerConfig) -> Result<String, String> {
        // S3 的「目录」是相对键前缀，空串表示桶根（即配置的 bucket/prefix）。
        Ok(String::new())
    }
    fn scan_dir(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        dir_url: &str,
        progress: Option<ScanProgress>,
    ) -> Result<Vec<ScannedSong>, String> {
        // 目录浏览为快速模式（文件名入库），标签由后台补全命令补齐。
        s3::scan_prefix(
            config,
            dir_url,
            cover_cache,
            existing_modified,
            progress,
            false,
        )
    }
    fn probe_metadata(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        song_url: &str,
    ) -> Option<ScannedSong> {
        s3::probe_song(config, cover_cache, song_url)
    }
    fn cache_song(
        &self,
        config: &StreamServerConfig,
        cache_root: &std::path::Path,
        server_id: &str,
        song_id: &str,
    ) -> Result<Option<String>, String> {
        s3::cache_song(config, cache_root, server_id, song_id).map(Some)
    }
}

impl SourceConnector for UpnpConnector {
    fn id(&self) -> &'static str {
        "upnp"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_upnp()
    }
    fn capabilities(&self) -> SourceCapabilities {
        // DLNA 的资源 `<res>` 本身就是 HTTP 直链：支持 Range、可缓存、可浏览；
        // 但服务器不提供写入，也没有远程歌单 / 歌词。
        SourceCapabilities::from_slice(&[
            SourceCapabilities::BROWSE,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::CACHEABLE,
            SourceCapabilities::READONLY,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        upnp::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        _existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        _read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        upnp::fetch_all_songs(config, progress)
    }
    fn get_stream_url(&self, _config: &StreamServerConfig, song_id: &str) -> String {
        // 库内 song_id 就是 DLNA 的资源直链，播放时原样交给 HTTP 引擎。
        song_id.to_string()
    }
    fn list_dir(
        &self,
        config: &StreamServerConfig,
        dir_url: Option<&str>,
    ) -> Result<Vec<DirEntry>, String> {
        upnp::list_dir(config, dir_url)
    }
    fn fetch_bytes(&self, config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
        upnp::fetch_bytes(config, url)
    }
    fn root_dir_url(&self, config: &StreamServerConfig) -> Result<String, String> {
        // DLNA 的「目录」是 ContentDirectory 的 ObjectID，根为 "0"。
        Ok(upnp::root_object_id(config))
    }
    fn scan_dir(
        &self,
        config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        dir_url: &str,
        _progress: Option<ScanProgress>,
    ) -> Result<Vec<ScannedSong>, String> {
        upnp::scan_dir(config, existing_modified, dir_url)
    }
    fn probe_metadata(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        song_url: &str,
    ) -> Option<ScannedSong> {
        upnp::probe_song(config, cover_cache, song_url)
    }
    fn cache_song(
        &self,
        config: &StreamServerConfig,
        cache_root: &std::path::Path,
        server_id: &str,
        song_id: &str,
    ) -> Result<Option<String>, String> {
        upnp::cache_song(config, cache_root, server_id, song_id).map(Some)
    }
}

impl SourceConnector for FtpConnector {
    fn id(&self) -> &'static str {
        "ftp"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_ftp()
    }
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::from_slice(&[
            SourceCapabilities::BROWSE,
            SourceCapabilities::INCREMENTAL,
            // FTP 无 HTTP Range，但经本地桥接后对播放器表现为可 Range、可 seek。
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::CACHEABLE,
            SourceCapabilities::WRITABLE,
            SourceCapabilities::SERVER_LYRICS,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        ftp::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        ftp::fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        match bridge::register(config, song_id) {
            Ok(url) => url,
            Err(e) => {
                log::error!("FTP 建立本地桥接失败: {e}");
                String::new()
            }
        }
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        ftp::get_lyrics(config, song_id)
    }
    fn list_dir(
        &self,
        config: &StreamServerConfig,
        dir_url: Option<&str>,
    ) -> Result<Vec<DirEntry>, String> {
        ftp::list_dir(config, dir_url)
    }
    fn fetch_bytes(&self, config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
        ftp::fetch_bytes(config, url)
    }
    fn upload(&self, config: &StreamServerConfig, local: &str, remote: &str) -> Result<(), String> {
        ftp::upload(config, local, remote)
    }
    fn delete(&self, config: &StreamServerConfig, url: &str) -> Result<(), String> {
        ftp::delete(config, url)
    }
    fn move_entry(
        &self,
        config: &StreamServerConfig,
        source: &str,
        destination: &str,
    ) -> Result<(), String> {
        ftp::move_entry(config, source, destination)
    }
    fn root_dir_url(&self, config: &StreamServerConfig) -> Result<String, String> {
        Ok(ftp::root_dir_url(config))
    }
    fn scan_dir(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        dir_url: &str,
        progress: Option<ScanProgress>,
    ) -> Result<Vec<ScannedSong>, String> {
        // 目录浏览为快速模式；标签交给后台补全命令（带节流）。
        ftp::scan_dir(
            config,
            cover_cache,
            existing_modified,
            dir_url,
            progress,
            false,
        )
    }
    fn probe_metadata(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        song_url: &str,
    ) -> Option<ScannedSong> {
        ftp::probe_song(config, cover_cache, song_url)
    }
    fn cache_song(
        &self,
        config: &StreamServerConfig,
        cache_root: &std::path::Path,
        server_id: &str,
        song_id: &str,
    ) -> Result<Option<String>, String> {
        ftp::cache_song(config, cache_root, server_id, song_id).map(Some)
    }
    fn open_reader(
        &self,
        config: &StreamServerConfig,
        song_id: &str,
    ) -> Result<Box<dyn bridge::RangeReader>, String> {
        ftp::open_reader(config, song_id)
    }
}

impl SourceConnector for LocalConnector {
    fn id(&self) -> &'static str {
        "smb"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_smb()
    }
    fn capabilities(&self) -> SourceCapabilities {
        // 本地/挂载路径由系统文件系统提供随机读，天然支持 seek；
        // 缓存无意义（源本身就在本机），因此不声明 CACHEABLE。
        SourceCapabilities::from_slice(&[
            SourceCapabilities::BROWSE,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::WRITABLE,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        localfs::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        localfs::fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        // 本地文件无需桥接：`AudioDecoder` 直接 `File::open`，且 `File` 天然可 seek。
        match localfs::resolve_path(config, song_id) {
            Ok(path) => path,
            Err(e) => {
                log::error!("解析本地路径失败: {e}");
                String::new()
            }
        }
    }
    fn list_dir(
        &self,
        config: &StreamServerConfig,
        dir_url: Option<&str>,
    ) -> Result<Vec<DirEntry>, String> {
        localfs::list_dir(config, dir_url)
    }
    fn fetch_bytes(&self, config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
        localfs::fetch_bytes(config, url)
    }
    fn upload(&self, config: &StreamServerConfig, local: &str, remote: &str) -> Result<(), String> {
        localfs::upload(config, local, remote)
    }
    fn delete(&self, config: &StreamServerConfig, url: &str) -> Result<(), String> {
        localfs::delete(config, url)
    }
    fn move_entry(
        &self,
        config: &StreamServerConfig,
        source: &str,
        destination: &str,
    ) -> Result<(), String> {
        localfs::move_entry(config, source, destination)
    }
    fn root_dir_url(&self, config: &StreamServerConfig) -> Result<String, String> {
        Ok(localfs::root_path(config)?
            .to_string_lossy()
            .to_string())
    }
    fn scan_dir(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        dir_url: &str,
        progress: Option<ScanProgress>,
    ) -> Result<Vec<ScannedSong>, String> {
        // 本地 IO 廉价：直接解析标签，无需"快速扫描 + 后台补全"两阶段。
        localfs::scan_dir(config, cover_cache, existing_modified, dir_url, progress, true)
    }
    fn probe_metadata(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        song_url: &str,
    ) -> Option<ScannedSong> {
        let path = localfs::resolve_path(config, song_url).ok()?;
        localfs::probe_song(&path, cover_cache)
    }
}

impl SourceConnector for PlexConnector {
    fn id(&self) -> &'static str {
        "plex"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_plex()
    }
    fn capabilities(&self) -> SourceCapabilities {
        // 与 Jellyfin/Emby 同族的"媒体服务器"形态：库扫描 + 只读。
        // Plex 没有目录浏览语义（不声明 BROWSE），也没有侧车回写；
        // 歌词与远程歌单不在本期范围（Plex 的歌词接口在各版本间不稳定）。
        //
        // 与 Jellyfin 的一处差异：Plex 的 `<Part key="/library/parts/…">`
        // 是**静态文件**，Range 语义明确，因此声明 RANGE_STREAMING；
        // Jellyfin 的 `/Audio/{id}/universal` 是转码端点，未声明该项。
        SourceCapabilities::from_slice(&[
            SourceCapabilities::SCAN_LIBRARY,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::READONLY,
            SourceCapabilities::TRUSTED_HOST,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        plex::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        _cover_cache: Option<&crate::utils::cover::CoverCache>,
        _existing_modified: Option<&HashMap<String, i64>>,
        _progress: Option<ScanProgress>,
        _read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        plex::fetch_all_songs(config)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        plex::get_stream_url(config, song_id)
    }
}

impl SourceConnector for FnMusicConnector {
    fn id(&self) -> &'static str {
        "fnmusic"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_fnmusic()
    }
    fn capabilities(&self) -> SourceCapabilities {
        // 飞牛音乐：库扫描 + 服务器歌词，只读。
        // 播放经本地桥接（`authx` 含 nonce/timestamp，必须每次请求前重签，
        // 静态头会在长时间播放后失效），因此声明 RANGE_STREAMING（对播放器
        // 表现为可 Range、可 seek）。
        SourceCapabilities::from_slice(&[
            SourceCapabilities::SCAN_LIBRARY,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::SERVER_LYRICS,
            SourceCapabilities::READONLY,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        fnmusic::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        fnmusic::fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        fnmusic::get_stream_url(config, song_id)
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        fnmusic::get_lyrics(config, song_id)
    }
    fn open_reader(
        &self,
        config: &StreamServerConfig,
        song_id: &str,
    ) -> Result<Box<dyn bridge::RangeReader>, String> {
        fnmusic::open_reader(config, song_id)
    }
}

impl SourceConnector for DaoLiYuConnector {
    fn id(&self) -> &'static str {
        "daoliyu"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_daoliyu()
    }
    fn capabilities(&self) -> SourceCapabilities {
        // 道理鱼：库扫描 + 服务器歌词，只读。
        // 播放走 `/api/tracks/{id}/download`，Bearer 需动态刷新，故经本地桥接
        // （token 不入播放引擎，由桥接在每次请求前重算头）。
        SourceCapabilities::from_slice(&[
            SourceCapabilities::SCAN_LIBRARY,
            SourceCapabilities::INCREMENTAL,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::SERVER_LYRICS,
            SourceCapabilities::READONLY,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        daoliyu::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        daoliyu::fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        daoliyu::get_stream_url(config, song_id)
    }
    fn get_lyrics(&self, config: &StreamServerConfig, song_id: &str) -> Option<String> {
        daoliyu::get_lyrics(config, song_id)
    }
    fn open_reader(
        &self,
        config: &StreamServerConfig,
        song_id: &str,
    ) -> Result<Box<dyn bridge::RangeReader>, String> {
        daoliyu::open_reader(config, song_id)
    }
}

impl SourceConnector for Pan115Connector {
    fn id(&self) -> &'static str {
        "pan115"
    }
    fn matches(&self, config: &StreamServerConfig) -> bool {
        config.is_pan115()
    }
    fn capabilities(&self) -> SourceCapabilities {
        // 115：目录 BFS 扫描 + HTTP 原生直链。
        // 直链本身支持 Range，但**有时效**，过期后需重新换链；故不声明缓存写透，
        // 避免把临时签名的字节当成永久缓存。
        SourceCapabilities::from_slice(&[
            SourceCapabilities::SCAN_LIBRARY,
            SourceCapabilities::RANGE_STREAMING,
            SourceCapabilities::READONLY,
            SourceCapabilities::TRUSTED_HOST,
        ])
    }
    fn test_connection(&self, config: &StreamServerConfig) -> ConnectionTestResult {
        pan115::test_connection(config)
    }
    fn fetch_all_songs(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
        existing_modified: Option<&HashMap<String, i64>>,
        progress: Option<ScanProgress>,
        read_tags: bool,
    ) -> Result<Vec<ScannedSong>, String> {
        pan115::fetch_all_songs(config, cover_cache, existing_modified, progress, read_tags)
    }
    fn get_stream_url(&self, config: &StreamServerConfig, song_id: &str) -> String {
        pan115::get_stream_url(config, song_id)
    }
    fn get_stream_play_info(
        &self,
        config: &StreamServerConfig,
        song_id: &str,
    ) -> StreamPlayInfo {
        // 115 直链对 UA 敏感，必须带上换链时使用的 UA。
        StreamPlayInfo {
            url: pan115::get_stream_url(config, song_id),
            headers: Some(pan115::stream_headers()),
        }
    }
}

/// 构建默认注册表（内置连接器）。
///
/// 后续新增音乐源时，在此追加注册即可。
pub fn registry() -> SourceRegistry {
    let mut reg = SourceRegistry::new();
    reg.register(Box::new(SubsonicConnector));
    reg.register(Box::new(WebdavConnector));
    reg.register(Box::new(JellyfinConnector));
    reg.register(Box::new(S3Connector));
    reg.register(Box::new(UpnpConnector));
    reg.register(Box::new(FtpConnector));
    reg.register(Box::new(LocalConnector));
    reg.register(Box::new(PlexConnector));
    reg.register(Box::new(FnMusicConnector));
    reg.register(Box::new(DaoLiYuConnector));
    reg.register(Box::new(Pan115Connector));
    reg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ServerType;

    fn config_of(t: ServerType) -> StreamServerConfig {
        StreamServerConfig {
            server_type: t,
            server_name: "t".to_string(),
            server_url: "https://example.com".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        }
    }

    /// 各内置连接器的能力位必须与"命令层门禁"的预期一致。
    #[test]
    fn capability_bits_match_connector_contract() {
        let webdav = config_of(ServerType::Webdav);
        assert!(supports(&webdav, SourceCapabilities::BROWSE));
        assert!(supports(&webdav, SourceCapabilities::WRITABLE));
        assert!(supports(&webdav, SourceCapabilities::CACHEABLE));
        assert!(supports(&webdav, SourceCapabilities::SIDECAR_WRITABLE));
        assert!(!supports(&webdav, SourceCapabilities::SERVER_PLAYLISTS));

        let s3 = config_of(ServerType::S3);
        assert!(supports(&s3, SourceCapabilities::BROWSE));
        assert!(supports(&s3, SourceCapabilities::CACHEABLE));
        assert!(supports(&s3, SourceCapabilities::RANGE_STREAMING));
        // S3 为只读对象存储：不可写、无侧车回写、无远程歌单。
        assert!(!supports(&s3, SourceCapabilities::WRITABLE));
        assert!(!supports(&s3, SourceCapabilities::SIDECAR_WRITABLE));
        assert!(!supports(&s3, SourceCapabilities::SERVER_PLAYLISTS));

        let subsonic = config_of(ServerType::Navidrome);
        assert!(supports(&subsonic, SourceCapabilities::SERVER_PLAYLISTS));
        assert!(!supports(&subsonic, SourceCapabilities::BROWSE));
    }

    #[test]
    fn capability_validate_has_no_unregistered_bits() {
        for t in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::OpenSubsonic,
            ServerType::Jellyfin,
            ServerType::Emby,
            ServerType::Webdav,
            ServerType::S3,
            ServerType::Upnp,
            ServerType::Ftp,
            ServerType::Smb,
            ServerType::Plex,
            ServerType::FnMusic,
            ServerType::DaoLiYu,
        ] {
            let cfg = config_of(t);
            assert_eq!(
                capabilities_for(&cfg).validate(),
                None,
                "连接器能力位含未登记位: {:?}",
                cfg.server_type
            );
        }
    }

    #[test]
    fn require_reports_missing_capability() {
        let subsonic = config_of(ServerType::Navidrome);
        assert!(require(&subsonic, SourceCapabilities::BROWSE, "目录浏览").is_err());
        assert!(require(&subsonic, SourceCapabilities::SERVER_PLAYLISTS, "远程歌单").is_ok());
        let webdav = config_of(ServerType::Webdav);
        assert!(require(&webdav, SourceCapabilities::WRITABLE, "删除").is_ok());
        assert!(require(&webdav, SourceCapabilities::SERVER_PLAYLISTS, "远程歌单").is_err());
    }

    /// 非文件型来源的默认实现必须"安全拒绝"，而不是 panic 或误报成功。
    #[test]
    fn non_browse_source_rejects_file_ops() {
        let cfg = config_of(ServerType::Navidrome);
        let conn = SubsonicConnector;
        assert!(conn.list_dir(&cfg, None).is_err());
        assert!(conn.fetch_bytes(&cfg, "x").is_err());
        assert!(conn.upload(&cfg, "a", "b").is_err());
        assert!(conn.delete(&cfg, "x").is_err());
        assert!(conn.move_entry(&cfg, "a", "b").is_err());
        assert!(conn.root_dir_url(&cfg).is_err());
        assert!(conn.scan_dir(&cfg, None, None, "", None).is_err());
        assert!(conn.probe_metadata(&cfg, None, "x").is_none());
        // 通用目录封面默认实现：无 BROWSE 能力直接返回 None（不发起任何请求）。
        assert!(conn.folder_cover(&cfg, "s", "x.mp3", None).is_none());
        assert_eq!(
            conn.cache_song(&cfg, std::path::Path::new("."), "s", "x")
                .unwrap(),
            None
        );
    }

    #[test]
    fn s3_root_dir_url_is_bucket_root() {
        let conn = S3Connector;
        assert_eq!(conn.root_dir_url(&config_of(ServerType::S3)).unwrap(), "");
    }

    /// FTP 不支持 HTTP，必须经本地桥接暴露成 Range 流；同时不可误声明 SERVER_PLAYLISTS。
    #[test]
    fn ftp_goes_through_loopback_bridge() {
        let cfg = config_of(ServerType::Ftp);
        let caps = capabilities_for(&cfg);
        assert!(caps.has(SourceCapabilities::BROWSE));
        assert!(caps.has(SourceCapabilities::RANGE_STREAMING));
        assert!(caps.has(SourceCapabilities::WRITABLE));
        assert!(caps.has(SourceCapabilities::CACHEABLE));
        assert!(!caps.has(SourceCapabilities::SERVER_PLAYLISTS));

        let url = FtpConnector.get_stream_url(&cfg, "album/song.flac");
        assert!(
            url.starts_with("http://127.0.0.1:"),
            "FTP 播放 URL 必须指向本地桥接，实际 {url}"
        );
        // URL 末段保留扩展名，供解码器给出 symphonia 探测提示。
        assert!(url.ends_with("song.flac"), "{url}");
        // seek 时会对同一 URL 再次发 Range 请求，因此必须可重复注册且结果稳定。
        assert_eq!(url, FtpConnector.get_stream_url(&cfg, "album/song.flac"));
    }

    /// 本地/挂载路径不需要桥接：直接返回本地路径，由解码器 `File::open`。
    #[test]
    fn local_mount_plays_directly_without_bridge() {
        let mut cfg = config_of(ServerType::Smb);
        cfg.server_url = "/mnt/nas".to_string();

        let caps = capabilities_for(&cfg);
        assert!(caps.has(SourceCapabilities::BROWSE));
        assert!(caps.has(SourceCapabilities::RANGE_STREAMING));
        assert!(!caps.has(SourceCapabilities::CACHEABLE), "本机文件无需再缓存");

        // 绝对路径原样返回。
        assert_eq!(
            LocalConnector.get_stream_url(&cfg, "/mnt/nas/a/b.flac"),
            "/mnt/nas/a/b.flac"
        );
        // 相对路径拼到配置根目录下（分隔符随平台，Windows 为 `\`）。
        let joined = LocalConnector.get_stream_url(&cfg, "a/b.flac");
        assert_eq!(
            std::path::Path::new(&joined),
            std::path::Path::new("/mnt/nas/a/b.flac"),
            "相对路径应拼到配置根目录下，实际 {joined}"
        );
        assert_eq!(
            std::path::Path::new(&LocalConnector.root_dir_url(&cfg).unwrap()),
            std::path::Path::new("/mnt/nas")
        );
    }

    /// SMB / FTP 的节流策略必须比局域网 WebDAV 更保守或相当，避免打爆对端。
    #[test]
    fn throttle_policy_covers_new_sources() {
        assert!(throttle::policy_for(&config_of(ServerType::S3)).max_items > 0);
        assert!(throttle::policy_for(&config_of(ServerType::Ftp)).max_items > 0);
        assert!(throttle::policy_for(&config_of(ServerType::Upnp)).max_items > 0);
        // 115 有 IP 频控，必须限速。
        assert!(throttle::policy_for(&config_of(ServerType::Pan115)).max_items > 0);
        // 本地挂载是纯本机 IO，不应被限速。
        assert_eq!(throttle::policy_for(&config_of(ServerType::Smb)).interval_ms, 0);
    }

    #[test]
    fn registry_resolves_every_builtin_type() {
        let reg = registry();
        for t in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::OpenSubsonic,
            ServerType::Jellyfin,
            ServerType::Emby,
            ServerType::Webdav,
            ServerType::S3,
            ServerType::Upnp,
            ServerType::Ftp,
            ServerType::Smb,
            ServerType::Plex,
            ServerType::FnMusic,
            ServerType::DaoLiYu,
            ServerType::Pan115,
        ] {
            let cfg = config_of(t);
            assert!(
                reg.resolve(&cfg).is_ok(),
                "连接器未覆盖: {:?}",
                cfg.server_type
            );
        }
    }

    /// Plex 是「媒体服务器」形态：可扫库、只读，但**没有**目录浏览，
    /// 也不能误声明侧车回写 / 远程歌单（命令层据此隐藏写操作入口）。
    #[test]
    fn plex_is_a_readonly_library_source() {
        let cfg = config_of(ServerType::Plex);
        let caps = capabilities_for(&cfg);
        assert!(caps.has(SourceCapabilities::SCAN_LIBRARY));
        assert!(caps.has(SourceCapabilities::READONLY));
        assert!(caps.has(SourceCapabilities::TRUSTED_HOST));
        assert!(!caps.has(SourceCapabilities::BROWSE));
        assert!(!caps.has(SourceCapabilities::WRITABLE));
        assert!(!caps.has(SourceCapabilities::SIDECAR_WRITABLE));
        assert!(!caps.has(SourceCapabilities::SERVER_PLAYLISTS));
        assert!(!caps.has(SourceCapabilities::SERVER_LYRICS));
    }

    /// Plex 的 `<Part key>` 就是 HTTP 路径 → **不需要**本地桥接。
    #[test]
    fn plex_plays_directly_without_bridge() {
        let mut cfg = config_of(ServerType::Plex);
        cfg.server_url = "http://192.168.1.10:32400".to_string();
        cfg.access_token = Some("TOK".to_string());

        let caps = capabilities_for(&cfg);
        assert!(caps.has(SourceCapabilities::RANGE_STREAMING));

        let url = PlexConnector.get_stream_url(&cfg, "/library/parts/9/1/file.flac");
        assert!(
            url.starts_with("http://192.168.1.10:32400/"),
            "Plex 应为 HTTP 原生直链，实际 {url}"
        );
        assert!(url.contains("X-Plex-Token=TOK"), "{url}");
        // 不能把 HTTP 源误丢进本地桥接。
        assert!(!url.contains("127.0.0.1"), "{url}");
    }
}
