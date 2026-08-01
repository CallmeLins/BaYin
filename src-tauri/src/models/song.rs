use serde::{Deserialize, Serialize};

/// 扫描到的歌曲信息，与前端 ScannedSong 接口一一对应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub file_path: String,
    pub file_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    /// 本地封面缓存 hash（WebDAV 等从标签提取并缓存的封面）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_hash: Option<String>,
    /// 远程文件修改时间（unix 秒），用于增量同步
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_modified: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_sq: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

/// 扫描选项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    pub directories: Vec<String>,
    #[serde(default)]
    pub skip_short_audio: Option<bool>,
    #[serde(default)]
    pub min_duration: Option<f64>,
}
