//! 流媒体服务器数据模型（支持 Navidrome/Subsonic/Jellyfin/Emby 等）
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn into_vec(self) -> Vec<T> {
        match self {
            OneOrMany::One(v) => vec![v],
            OneOrMany::Many(v) => v,
        }
    }
}

/// 服务器类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServerType {
    Navidrome,
    Subsonic,
    #[serde(rename = "opensubsonic")]
    OpenSubsonic,
    Jellyfin,
    Emby,
}

/// 统一流媒体服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamServerConfig {
    pub server_type: ServerType,
    pub server_name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

impl StreamServerConfig {
    /// 是否使用 Subsonic API（Navidrome/Subsonic/OpenSubsonic）
    pub fn is_subsonic(&self) -> bool {
        matches!(
            self.server_type,
            ServerType::Navidrome | ServerType::Subsonic | ServerType::OpenSubsonic
        )
    }

    /// 是否使用 Jellyfin/Emby API
    pub fn is_jellyfin_like(&self) -> bool {
        matches!(self.server_type, ServerType::Jellyfin | ServerType::Emby)
    }
}

/// 连接测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
}

// ============ Subsonic API 模型 ============

/// Subsonic API 响应包装
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponse<T> {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: SubsonicResponseInner<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponseInner<T> {
    pub status: String,
    pub version: String,
    #[serde(flatten)]
    pub data: Option<T>,
    pub error: Option<SubsonicError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicError {
    pub message: String,
}

/// ping 响应（用于测试连接）
#[derive(Debug, Deserialize)]
pub struct PingResponse {}

/// 搜索响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub search_result3: Option<SearchResult3>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3 {
    pub song: Option<Vec<SubsonicSong>>,
}

/// Subsonic 歌曲信息
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicSong {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub bit_rate: Option<u32>,
    #[serde(default)]
    pub sampling_rate: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<u8>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
}

// ============ Jellyfin/Emby API 模型 ============

/// Jellyfin 认证请求
// ============ Subsonic playlists ============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicPlaylistsResponse {
    pub playlists: Option<SubsonicPlaylists>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicPlaylists {
    pub playlist: Option<OneOrMany<SubsonicPlaylistSummary>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicPlaylistSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub song_count: Option<u32>,
    #[serde(default)]
    pub changed: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicPlaylistResponse {
    pub playlist: Option<SubsonicPlaylist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicPlaylist {
    #[serde(default)]
    pub entry: Option<OneOrMany<SubsonicPlaylistEntry>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicPlaylistEntry {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub cover_art: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinAuthRequest {
    pub username: String,
    pub pw: String,
}

/// Jellyfin 认证响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinAuthResponse {
    pub access_token: String,
    pub user: JellyfinUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUser {
    pub id: String,
}

/// Jellyfin 系统信息响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinSystemInfo {
    pub version: Option<String>,
}

/// Jellyfin Items 查询响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinItemsResponse {
    pub items: Vec<JellyfinItem>,
    pub total_record_count: u64,
}

/// Jellyfin 媒体项
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub playlist_item_id: Option<String>,
    #[serde(default)]
    pub child_count: Option<u32>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub album_artist: Option<String>,
    #[serde(default, rename = "Artists")]
    pub artists: Option<Vec<String>>,
    #[serde(default)]
    pub run_time_ticks: Option<u64>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub image_tags: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub media_sources: Option<Vec<JellyfinMediaSource>>,
    #[serde(default)]
    pub date_created: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinMediaSource {
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub media_streams: Option<Vec<JellyfinMediaStream>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinMediaStream {
    #[serde(default, rename = "Type")]
    pub stream_type: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<u8>,
    #[serde(default)]
    pub channels: Option<u32>,
}

/// Jellyfin 歌词响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinLyricsResponse {
    pub lyrics: Vec<JellyfinLyricLine>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinLyricLine {
    #[serde(default)]
    pub start: Option<u64>,
    pub text: String,
}
