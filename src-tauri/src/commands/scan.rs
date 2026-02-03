//! Advanced scanning commands with incremental scan and progress events

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use tauri::{AppHandle, Emitter, State};
use walkdir::WalkDir;

use crate::commands::CoverCacheState;
use crate::db::{self, DbState, SongInput};
use crate::models::{
    LocalScanOptions, ScanMode, ScanPhase, ScanProgress, ScanResult, StreamScanOptions,
};
use crate::utils::audio::{is_audio_file, read_metadata_with_mtime};
use crate::utils::cover::extract_and_cache_cover;

/// Emit scan progress event
fn emit_progress(app: &AppHandle, progress: &ScanProgress) {
    let _ = app.emit("scan-progress", progress);
}

/// Scan local directories to database with progress events
#[tauri::command]
pub async fn scan_local_to_db(
    app: AppHandle,
    db: State<'_, DbState>,
    cover_cache: State<'_, CoverCacheState>,
    options: LocalScanOptions,
) -> Result<ScanResult, String> {
    let start_time = Instant::now();
    let min_duration = options.min_duration.unwrap_or(0.0);
    let batch_size = options.batch_size;

    // Get cover cache for use in parallel processing
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?.clone_arc();

    // Phase 1: Collect all audio file paths
    emit_progress(
        &app,
        &ScanProgress {
            phase: ScanPhase::Collecting,
            total: 0,
            processed: 0,
            current_file: None,
            skipped: 0,
            errors: 0,
        },
    );

    let mut audio_paths: Vec<PathBuf> = Vec::new();

    for dir in &options.directories {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            continue;
        }

        for entry in WalkDir::new(dir_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && is_audio_file(path) {
                audio_paths.push(path.to_path_buf());
            }
        }
    }

    let total_files = audio_paths.len();

    // Phase 2: Check which files need scanning (for incremental mode)
    let files_to_scan: Vec<PathBuf>;
    let mut skipped_count = 0;

    match options.mode {
        ScanMode::Incremental => {
            emit_progress(
                &app,
                &ScanProgress {
                    phase: ScanPhase::Checking,
                    total: total_files,
                    processed: 0,
                    current_file: None,
                    skipped: 0,
                    errors: 0,
                },
            );

            // Get existing files from DB with their modification times
            let existing_files: HashMap<String, Option<i64>> = {
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                let songs = db::songs::get_all_songs(&conn).map_err(|e| e.to_string())?;
                songs
                    .into_iter()
                    .filter(|s| s.source_type == "local")
                    .map(|s| (s.file_path, s.file_modified))
                    .collect()
            };

            // Filter to only files that are new or modified
            files_to_scan = audio_paths
                .into_iter()
                .filter(|path| {
                    let path_str = path.to_string_lossy().to_string();
                    match existing_files.get(&path_str) {
                        Some(Some(db_mtime)) => {
                            // File exists in DB, check if modified
                            match std::fs::metadata(path) {
                                Ok(meta) => match meta.modified() {
                                    Ok(mtime) => {
                                        let file_mtime = mtime
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);
                                        if file_mtime > *db_mtime {
                                            true // File modified, rescan
                                        } else {
                                            skipped_count += 1;
                                            false // File unchanged, skip
                                        }
                                    }
                                    Err(_) => true,
                                },
                                Err(_) => true,
                            }
                        }
                        Some(None) => true, // No mtime in DB, rescan
                        None => true,       // New file
                    }
                })
                .collect();
        }
        ScanMode::Full => {
            files_to_scan = audio_paths;
        }
    }

    let files_to_process = files_to_scan.len();

    // Phase 3: Read metadata in parallel
    emit_progress(
        &app,
        &ScanProgress {
            phase: ScanPhase::Scanning,
            total: files_to_process,
            processed: 0,
            current_file: None,
            skipped: skipped_count,
            errors: 0,
        },
    );

    let processed_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let cache_clone = cache.clone();

    let songs: Vec<SongInput> = files_to_scan
        .par_iter()
        .filter_map(|path| {
            let result = read_metadata_with_mtime(path);
            let processed = processed_count.fetch_add(1, Ordering::Relaxed) + 1;

            // Emit progress every 50 files
            if processed % 50 == 0 || processed == files_to_process {
                let _ = app.emit(
                    "scan-progress",
                    ScanProgress {
                        phase: ScanPhase::Scanning,
                        total: files_to_process,
                        processed,
                        current_file: Some(path.to_string_lossy().to_string()),
                        skipped: skipped_count,
                        errors: error_count.load(Ordering::Relaxed),
                    },
                );
            }

            match result {
                Ok(song) => {
                    // Skip short audio if configured
                    if min_duration > 0.0 && song.duration < min_duration {
                        return None;
                    }

                    // Extract and cache cover, get hash
                    let cover_hash = extract_and_cache_cover(path, &cache_clone).ok().flatten();

                    Some(SongInput {
                        id: song.id,
                        title: song.title,
                        artist: song.artist,
                        album: song.album,
                        duration: song.duration,
                        file_path: song.file_path,
                        file_size: song.file_size as i64,
                        is_hr: song.is_hr,
                        is_sq: song.is_sq,
                        cover_hash, // Store hash instead of base64
                        server_song_id: None,
                        stream_info: None,
                        file_modified: Some(song.file_modified),
                    })
                }
                Err(_) => {
                    error_count.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
        })
        .collect();

    let errors = error_count.load(Ordering::Relaxed);

    // Phase 4: Save to database in batches
    emit_progress(
        &app,
        &ScanProgress {
            phase: ScanPhase::Saving,
            total: songs.len(),
            processed: 0,
            current_file: None,
            skipped: skipped_count,
            errors,
        },
    );

    let added_count;
    {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;

        // For full scan, clear local songs first
        if matches!(options.mode, ScanMode::Full) {
            db::songs::delete_songs_by_source(&conn, "local", None).map_err(|e| e.to_string())?;
        }

        // Save in batches
        let mut total_saved = 0;
        for chunk in songs.chunks(batch_size) {
            db::songs::save_songs(&mut conn, chunk, "local", None).map_err(|e| e.to_string())?;
            total_saved += chunk.len();

            emit_progress(
                &app,
                &ScanProgress {
                    phase: ScanPhase::Saving,
                    total: songs.len(),
                    processed: total_saved,
                    current_file: None,
                    skipped: skipped_count,
                    errors,
                },
            );
        }

        added_count = total_saved;
    }

    // Phase 5: Cleanup - remove songs whose files no longer exist
    let removed_count;
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;

        emit_progress(
            &app,
            &ScanProgress {
                phase: ScanPhase::Cleanup,
                total: 0,
                processed: 0,
                current_file: None,
                skipped: skipped_count,
                errors,
            },
        );

        // Get all local songs from DB
        let all_local_songs = db::songs::get_all_songs(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|s| s.source_type == "local")
            .collect::<Vec<_>>();

        // Find songs whose files no longer exist
        let missing_ids: Vec<String> = all_local_songs
            .iter()
            .filter(|s| !Path::new(&s.file_path).exists())
            .map(|s| s.id.clone())
            .collect();

        removed_count = missing_ids.len();

        // Delete missing songs
        for id in &missing_ids {
            conn.execute("DELETE FROM songs WHERE id = ?1", [id])
                .map_err(|e| e.to_string())?;
        }
    }

    // Get final count
    let total_songs = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_song_count_by_source(&conn, "local").map_err(|e| e.to_string())? as usize
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    // Phase 6: Complete
    emit_progress(
        &app,
        &ScanProgress {
            phase: ScanPhase::Complete,
            total: total_songs,
            processed: total_songs,
            current_file: None,
            skipped: skipped_count,
            errors,
        },
    );

    // Emit library-updated event
    let _ = app.emit("library-updated", ());

    Ok(ScanResult {
        total_songs,
        added: added_count,
        updated: 0, // TODO: track updates separately
        removed: removed_count,
        skipped: skipped_count,
        errors,
        duration_ms,
    })
}

/// Scan stream servers to database
#[tauri::command]
pub async fn scan_stream_to_db(
    app: AppHandle,
    db: State<'_, DbState>,
    options: StreamScanOptions,
) -> Result<ScanResult, String> {
    let start_time = Instant::now();

    emit_progress(
        &app,
        &ScanProgress {
            phase: ScanPhase::Collecting,
            total: 0,
            processed: 0,
            current_file: None,
            skipped: 0,
            errors: 0,
        },
    );

    // Get servers to scan
    let servers = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let all_servers = db::servers::get_stream_servers(&conn).map_err(|e| e.to_string())?;

        if let Some(server_id) = &options.server_id {
            all_servers
                .into_iter()
                .filter(|s| &s.id == server_id && s.enabled)
                .collect::<Vec<_>>()
        } else {
            all_servers.into_iter().filter(|s| s.enabled).collect()
        }
    };

    if servers.is_empty() {
        return Ok(ScanResult {
            total_songs: 0,
            added: 0,
            updated: 0,
            removed: 0,
            skipped: 0,
            errors: 0,
            duration_ms: start_time.elapsed().as_millis() as u64,
        });
    }

    let mut total_added = 0;
    let mut total_errors = 0;

    for server in &servers {
        emit_progress(
            &app,
            &ScanProgress {
                phase: ScanPhase::Scanning,
                total: 0,
                processed: 0,
                current_file: Some(server.server_name.clone()),
                skipped: 0,
                errors: total_errors,
            },
        );

        // Build config for fetching
        let config = crate::models::StreamServerConfig {
            server_type: match server.server_type.as_str() {
                "navidrome" => crate::models::ServerType::Navidrome,
                "subsonic" => crate::models::ServerType::Subsonic,
                "opensubsonic" => crate::models::ServerType::OpenSubsonic,
                "jellyfin" => crate::models::ServerType::Jellyfin,
                "emby" => crate::models::ServerType::Emby,
                _ => crate::models::ServerType::Navidrome,
            },
            server_name: server.server_name.clone(),
            server_url: server.server_url.clone(),
            username: server.username.clone(),
            password: server.password.clone(),
            access_token: server.access_token.clone(),
            user_id: server.user_id.clone(),
        };

        // Fetch songs from server
        let stream_songs = match crate::commands::streaming::fetch_stream_songs_internal(&config).await {
            Ok(songs) => songs,
            Err(e) => {
                total_errors += 1;
                eprintln!("Failed to fetch songs from {}: {}", server.server_name, e);
                continue;
            }
        };

        // Clear old songs for this server
        {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            db::songs::delete_songs_by_source(&conn, "stream", Some(&server.id))
                .map_err(|e| e.to_string())?;
        }

        // Convert to SongInput
        // Note: Stream songs don't cache covers locally, they use server URLs
        let song_inputs: Vec<SongInput> = stream_songs
            .iter()
            .map(|s| SongInput {
                id: format!("{}-{}", server.id, s.id),
                title: s.title.clone(),
                artist: s.artist.clone(),
                album: s.album.clone(),
                duration: s.duration,
                file_path: String::new(),
                file_size: s.file_size as i64,
                is_hr: s.is_hr,
                is_sq: s.is_sq,
                cover_hash: None, // Stream songs use server cover URLs directly
                server_song_id: Some(s.id.clone()),
                stream_info: Some(serde_json::json!({
                    "type": "stream",
                    "serverType": server.server_type,
                    "songId": s.id,
                    "serverName": server.server_name,
                    "coverUrl": s.cover_url, // Store cover URL in stream_info
                    "config": {
                        "serverType": server.server_type,
                        "serverName": server.server_name,
                        "serverUrl": server.server_url,
                        "username": server.username,
                        "password": server.password,
                        "accessToken": server.access_token,
                        "userId": server.user_id
                    }
                }).to_string()),
                file_modified: None,
            })
            .collect();

        // Save to database
        {
            let mut conn = db.0.lock().map_err(|e| e.to_string())?;
            let saved = db::songs::save_songs(&mut conn, &song_inputs, "stream", Some(&server.id))
                .map_err(|e| e.to_string())?;
            total_added += saved;
        }

        emit_progress(
            &app,
            &ScanProgress {
                phase: ScanPhase::Saving,
                total: stream_songs.len(),
                processed: stream_songs.len(),
                current_file: Some(server.server_name.clone()),
                skipped: 0,
                errors: total_errors,
            },
        );
    }

    // Get final count
    let total_songs = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_song_count_by_source(&conn, "stream").map_err(|e| e.to_string())? as usize
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    emit_progress(
        &app,
        &ScanProgress {
            phase: ScanPhase::Complete,
            total: total_songs,
            processed: total_songs,
            current_file: None,
            skipped: 0,
            errors: total_errors,
        },
    );

    // Emit library-updated event
    let _ = app.emit("library-updated", ());

    Ok(ScanResult {
        total_songs,
        added: total_added,
        updated: 0,
        removed: 0,
        skipped: 0,
        errors: total_errors,
        duration_ms,
    })
}
