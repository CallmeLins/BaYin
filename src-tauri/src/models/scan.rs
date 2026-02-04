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
    pub file_modified: i64,
}
