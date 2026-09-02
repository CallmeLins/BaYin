//! Scan-related models

use serde::{Deserialize, Serialize};

/// Scan mode
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    /// Full scan - rescan all files
    #[default]
    Full,
    /// Incremental scan - only scan new/modified files
    Incremental,
}

/// Scan progress event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// Current phase of scanning
    pub phase: ScanPhase,
    /// Total files to process
    pub total: usize,
    /// Files processed so far
    pub processed: usize,
    /// Current file being processed (if any)
    pub current_file: Option<String>,
    /// Number of files skipped (already up-to-date)
    pub skipped: usize,
    /// Number of files with errors
    pub errors: usize,
}

/// Scan phases
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanPhase {
    /// Collecting files from directories
    Collecting,
    /// 断点续扫：从上次保存的扫描状态继续（source/scan 与 scan_resume 表）
    Resuming,
    /// Checking for changes (incremental mode)
    Checking,
    /// Reading metadata
    Scanning,
    /// Writing to database
    Saving,
    /// Cleanup (removing deleted files from DB)
    Cleanup,
    /// Scan complete
    Complete,
}

/// Scan result summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Total songs in library after scan
    pub total_songs: usize,
    /// New songs added
    pub added: usize,
    /// Existing songs updated
    pub updated: usize,
    /// Songs removed (file no longer exists)
    pub removed: usize,
    /// Files skipped (unchanged)
    pub skipped: usize,
    /// Files that failed to scan
    pub errors: usize,
    /// Detailed failures from stream server scans
    #[serde(default)]
    pub stream_failures: Vec<String>,
    /// Time taken in milliseconds
    pub duration_ms: u64,
}

/// Scan options for local directories
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalScanOptions {
    /// Directories to scan
    pub directories: Vec<String>,
    /// Scan mode (full or incremental)
    #[serde(default)]
    pub mode: ScanMode,
    /// Skip audio files shorter than this duration (seconds)
    #[serde(default)]
    pub min_duration: Option<f64>,
    /// Batch size for database writes
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// 是否允许断点续扫：为 true 时，若存在未完成的 `scan_resume`
    /// 记录则从上次中断点继续（发出 Resuming 相位并复用上次收集到的
    /// 待处理文件清单），否则退化为普通扫描并建立新断点。
    #[serde(default)]
    pub resume: bool,
}

fn default_batch_size() -> usize {
    500
}

/// Scan options for stream servers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamScanOptions {
    /// Server ID to scan (if None, scan all enabled servers)
    pub server_id: Option<String>,
    /// 是否读取音频标签（true=完整扫描逐首探测；false/None=快速扫描仅文件名入库）
    pub read_tags: Option<bool>,
}

/// Extended song info with file modification time
#[derive(Debug, Clone)]
pub struct ScannedSongWithMtime {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub file_path: String,
    pub file_size: u64,
    pub is_hr: Option<bool>,
    pub is_sq: Option<bool>,
    pub format: Option<String>,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
    pub bitrate: Option<u32>,
    pub channels: Option<u8>,
    pub file_modified: i64,
}
