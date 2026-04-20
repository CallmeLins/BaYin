use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::models::{ScanOptions, ScannedSong};
use crate::utils::audio::{is_audio_file, read_lyrics, read_metadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub fn list_directories(path: String) -> Result<Vec<DirectoryEntry>, String> {
    let dir_path = Path::new(&path);

    if !dir_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    if !dir_path.is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    let mut entries = Vec::new();

    match fs::read_dir(dir_path) {
        Ok(read_dir) => {
            for entry in read_dir.filter_map(|item| item.ok()) {
                let entry_path = entry.path();
                if !entry_path.is_dir() {
                    continue;
                }

                let name = entry_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();

                if name.starts_with('.') {
                    continue;
                }

                entries.push(DirectoryEntry {
                    name,
                    path: entry_path.to_string_lossy().to_string(),
                    is_dir: true,
                });
            }
        }
        Err(err) => {
            return Err(format!("Failed to read directory: {}", err));
        }
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(entries)
}

pub fn scan_music_files(options: ScanOptions) -> Result<Vec<ScannedSong>, String> {
    let skip_short = options.skip_short_audio.unwrap_or(false);
    let min_duration = options.min_duration.unwrap_or(30.0);

    let mut audio_paths: Vec<PathBuf> = Vec::new();

    for dir in &options.directories {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            continue;
        }

        for entry in WalkDir::new(dir_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|item| item.ok())
        {
            let path = entry.path();
            if path.is_file() && is_audio_file(path) {
                audio_paths.push(path.to_path_buf());
            }
        }
    }

    let songs = audio_paths
        .par_iter()
        .filter_map(|path| match read_metadata(path) {
            Ok(song) if !skip_short || song.duration >= min_duration => Some(song),
            Ok(_) => None,
            Err(_) => None,
        })
        .collect();

    Ok(songs)
}

pub fn get_music_metadata(file_path: String) -> Result<Option<ScannedSong>, String> {
    let path = Path::new(&file_path);

    if !path.exists() || !path.is_file() || !is_audio_file(path) {
        return Ok(None);
    }

    match read_metadata(path) {
        Ok(song) => Ok(Some(song)),
        Err(_) => Ok(None),
    }
}

pub fn get_lyrics(file_path: String) -> Result<Option<String>, String> {
    let path = Path::new(&file_path);

    if !path.exists() || !path.is_file() {
        return Ok(None);
    }

    Ok(read_lyrics(path))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanAndSaveResult {
    pub scanned: u64,
    pub saved: u64,
    pub skipped: u64,
}

/// Scan the configured directories and upsert everything found as local songs.
///
/// Returns counts so the UI can surface progress / success toasts.
pub fn scan_and_save_music_files(options: ScanOptions) -> Result<ScanAndSaveResult, String> {
    use crate::db::{self, SongInput};
    use crate::state::with_db_mut;

    let scanned = scan_music_files(options)?;
    let scanned_count = scanned.len() as u64;

    let inputs: Vec<SongInput> = scanned
        .into_iter()
        .map(|s| SongInput {
            id: s.id,
            title: s.title,
            artist: s.artist,
            album: s.album,
            duration: s.duration,
            file_path: s.file_path,
            file_size: s.file_size as i64,
            is_hr: s.is_hr,
            is_sq: s.is_sq,
            cover_hash: None,
            server_song_id: None,
            stream_info: None,
            file_modified: None,
            format: s.format,
            bit_depth: s.bit_depth,
            sample_rate: s.sample_rate,
            bitrate: s.bitrate,
            channels: s.channels,
            created_at: s.created_at,
        })
        .collect();

    let saved = with_db_mut(|conn| {
        db::songs::save_songs(conn, &inputs, "local", None)
            .map(|count| count as u64)
            .map_err(|e| e.to_string())
    })?;

    Ok(ScanAndSaveResult {
        scanned: scanned_count,
        saved,
        skipped: scanned_count.saturating_sub(saved),
    })
}
