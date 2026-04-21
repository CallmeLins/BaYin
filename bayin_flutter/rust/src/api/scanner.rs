use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::models::{ScanOptions, ScannedSong};
use crate::state::{with_cover_cache, with_db_mut};
use crate::utils::audio::{is_audio_file, read_lyrics, read_metadata};
use crate::utils::cover::extract_and_cache_cover;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackfillCoversResult {
    pub total_candidates: u64,
    pub updated: u64,
    pub skipped: u64,
    pub failed: u64,
}

/// Scan the configured directories and upsert everything found as local songs.
///
/// Returns counts so the UI can surface progress / success toasts.
pub fn scan_and_save_music_files(options: ScanOptions) -> Result<ScanAndSaveResult, String> {
    use crate::db::{self, SongInput};

    let scanned = scan_music_files(options)?;
    let scanned_count = scanned.len() as u64;

    let inputs: Vec<SongInput> = scanned
        .into_iter()
        .map(|s| SongInput {
            cover_hash: with_cover_cache(|cache| {
                extract_and_cache_cover(Path::new(&s.file_path), cache)
            })
            .ok()
            .flatten(),
            id: s.id,
            title: s.title,
            artist: s.artist,
            album: s.album,
            duration: s.duration,
            file_path: s.file_path,
            file_size: s.file_size as i64,
            is_hr: s.is_hr,
            is_sq: s.is_sq,
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

pub fn backfill_song_covers() -> Result<BackfillCoversResult, String> {
    use rusqlite::params;

    with_db_mut(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, file_path
                 FROM songs
                 WHERE source_type = 'local' AND (cover_hash IS NULL OR cover_hash = '')",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let file_path: String = row.get(1)?;
                Ok((id, file_path))
            })
            .map_err(|e| e.to_string())?;

        let mut candidates = Vec::<(String, String)>::new();
        for row in rows {
            candidates.push(row.map_err(|e| e.to_string())?);
        }
        drop(stmt);

        let total_candidates = candidates.len() as u64;
        let mut updated = 0u64;
        let mut skipped = 0u64;
        let mut failed = 0u64;

        for (song_id, file_path) in candidates {
            let path = Path::new(&file_path);
            if !path.exists() || !path.is_file() {
                failed += 1;
                continue;
            }

            let cover_hash = match with_cover_cache(|cache| extract_and_cache_cover(path, cache)) {
                Ok(value) => value,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };

            let Some(hash) = cover_hash else {
                skipped += 1;
                continue;
            };

            conn.execute(
                "UPDATE songs
                 SET cover_hash = ?1, updated_at = strftime('%s','now')
                 WHERE id = ?2",
                params![hash, song_id],
            )
            .map_err(|e| e.to_string())?;
            updated += 1;
        }

        Ok(BackfillCoversResult {
            total_candidates,
            updated,
            skipped,
            failed,
        })
    })
}
