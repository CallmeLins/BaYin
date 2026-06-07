//! Database Tauri commands

use crate::db::{
    self, DbAlbum, DbArtist, DbSong, DbState, DbStreamServer, ScanConfig, SongInput,
    StreamServerInput,
};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Migration data from localStorage
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationData {
    pub songs: Vec<MigrationSong>,
    #[serde(default)]
    pub stream_config: Option<MigrationStreamConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub file_path: Option<String>,
    #[serde(default)]
    pub file_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_sq: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStreamConfig {
    pub server_type: String,
    pub server_name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_auth: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Get all songs from the database
#[tauri::command]
pub fn db_get_all_songs(db: State<'_, DbState>) -> Result<Vec<DbSong>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::get_all_songs(&conn).map_err(|e| e.to_string())
}

/// Get all albums (aggregated from songs)
#[tauri::command]
pub fn db_get_all_albums(db: State<'_, DbState>) -> Result<Vec<DbAlbum>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::albums::get_all_albums(&conn).map_err(|e| e.to_string())
}

/// Get all artists (aggregated from songs)
#[tauri::command]
pub fn db_get_all_artists(db: State<'_, DbState>) -> Result<Vec<DbArtist>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::albums::get_all_artists(&conn).map_err(|e| e.to_string())
}

/// Save songs to database
#[tauri::command]
pub fn db_save_songs(
    db: State<'_, DbState>,
    songs: Vec<SongInput>,
    source_type: String,
    server_id: Option<String>,
) -> Result<usize, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::save_songs(&mut conn, &songs, &source_type, server_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Delete songs by source type
#[tauri::command]
pub fn db_delete_songs_by_source(
    db: State<'_, DbState>,
    source_type: String,
    server_id: Option<String>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::delete_songs_by_source(&conn, &source_type, server_id.as_deref())
        .map_err(|e| e.to_string())
}

/// Clear all songs
#[tauri::command]
pub fn db_clear_all_songs(db: State<'_, DbState>) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::clear_all_songs(&conn).map_err(|e| e.to_string())
}

/// Get all stream servers
#[tauri::command]
pub fn db_get_stream_servers(db: State<'_, DbState>) -> Result<Vec<DbStreamServer>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::get_stream_servers(&conn).map_err(|e| e.to_string())
}

/// Save stream server configuration
#[tauri::command]
pub fn db_save_stream_server(
    db: State<'_, DbState>,
    config: StreamServerInput,
) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::save_stream_server(&conn, &config).map_err(|e| e.to_string())
}

/// Delete stream server and its associated songs
#[tauri::command]
pub fn db_delete_stream_server(db: State<'_, DbState>, server_id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::delete_stream_server(&conn, &server_id).map_err(|e| e.to_string())
}

/// Clear all stream servers
#[tauri::command]
pub fn db_clear_stream_servers(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::clear_stream_servers(&conn).map_err(|e| e.to_string())
}

/// Save scan configuration
#[tauri::command]
pub fn db_save_scan_config(db: State<'_, DbState>, config: ScanConfig) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::save_scan_config(&conn, &config).map_err(|e| e.to_string())
}

/// Get scan configuration
#[tauri::command]
pub fn db_get_scan_config(db: State<'_, DbState>) -> Result<Option<ScanConfig>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::get_scan_config(&conn).map_err(|e| e.to_string())
}

/// Clear scan configuration
#[tauri::command]
pub fn db_clear_scan_config(db: State<'_, DbState>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::clear_scan_config(&conn).map_err(|e| e.to_string())
}

/// Migrate data from localStorage (one-time migration)
#[tauri::command]
pub fn db_migrate_from_localstorage(
    db: State<'_, DbState>,
    data: MigrationData,
) -> Result<usize, String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;

    // Check if we have any existing songs
    let existing_count = db::songs::get_song_count(&conn).map_err(|e| e.to_string())?;
    if existing_count > 0 {
        return Ok(0); // Already have data, skip migration
    }

    // Separate local and stream songs
    let mut local_songs = Vec::new();
    let mut stream_songs = Vec::new();

    for song in data.songs {
        let file_path = song.file_path.unwrap_or_default();

        // Check if this is a stream song by parsing the filePath
        let is_stream = file_path.starts_with('{') && file_path.contains("\"type\":\"stream\"");

        let song_input = SongInput {
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            duration: song.duration,
            file_path: file_path.clone(),
            file_size: song.file_size.unwrap_or(0),
            is_hr: song.is_hr,
            is_sq: song.is_sq,
            cover_hash: None,
            server_song_id: None,
            stream_info: if is_stream { Some(file_path) } else { None },
            file_modified: None,
            format: None,
            bit_depth: None,
            sample_rate: None,
            bitrate: None,
            channels: None,
            created_at: None,
        };

        if is_stream {
            stream_songs.push(song_input);
        } else {
            local_songs.push(song_input);
        }
    }

    let mut total = 0;

    // Save local songs
    if !local_songs.is_empty() {
        total += db::songs::save_songs(&mut conn, &local_songs, "local", None)
            .map_err(|e| e.to_string())?;
    }

    // Save stream server config if present
    let server_id = if let Some(config) = data.stream_config {
        let input = StreamServerInput {
            server_type: config.server_type,
            server_name: config.server_name,
            server_url: config.server_url,
            username: config.username,
            password: config.password,
            legacy_auth: config.legacy_auth,
            music_folder_id: config.music_folder_id,
            access_token: config.access_token,
            user_id: config.user_id,
            enabled: None,
        };
        Some(db::servers::save_stream_server(&conn, &input).map_err(|e| e.to_string())?)
    } else {
        None
    };

    // Save stream songs
    if !stream_songs.is_empty() {
        total += db::songs::save_songs(&mut conn, &stream_songs, "stream", server_id.as_deref())
            .map_err(|e| e.to_string())?;
    }

    Ok(total)
}

/// Get library statistics
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub total_songs: i64,
    pub local_songs: i64,
    pub stream_songs: i64,
    pub total_albums: i64,
    pub total_artists: i64,
}

#[tauri::command]
pub fn db_get_library_stats(db: State<'_, DbState>) -> Result<LibraryStats, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let total_songs = db::songs::get_song_count(&conn).map_err(|e| e.to_string())?;
    let local_songs =
        db::songs::get_song_count_by_source(&conn, "local").map_err(|e| e.to_string())?;
    let stream_songs =
        db::songs::get_song_count_by_source(&conn, "stream").map_err(|e| e.to_string())?;

    let albums = db::albums::get_all_albums(&conn).map_err(|e| e.to_string())?;
    let artists = db::albums::get_all_artists(&conn).map_err(|e| e.to_string())?;

    Ok(LibraryStats {
        total_songs,
        local_songs,
        stream_songs,
        total_albums: albums.len() as i64,
        total_artists: artists.len() as i64,
    })
}

// ============ Cover Cache Commands ============

use crate::utils::cover::{CoverCache, CoverSize};
use std::sync::Mutex;

/// Cover cache state wrapper
pub struct CoverCacheState(pub Mutex<CoverCache>);

/// Get cover URL by cover hash and size
/// This is the primary method - frontend should use cover_hash from songs/albums
#[tauri::command]
pub fn get_cover_url(
    cover_cache: State<'_, CoverCacheState>,
    hash: String,
    size: Option<String>,
) -> Result<Option<String>, String> {
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?;

    let cover_size = match size.as_deref() {
        Some("small") | Some("list") => CoverSize::Small,
        Some("original") | Some("orig") => CoverSize::Original,
        _ => CoverSize::Mid,
    };

    Ok(cache.get_cover_url(&hash, cover_size))
}

/// Batch get cover URLs for multiple hashes
/// More efficient than calling get_cover_url multiple times
#[tauri::command]
pub fn get_cover_urls_batch(
    cover_cache: State<'_, CoverCacheState>,
    hashes: Vec<String>,
    size: Option<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?;

    let cover_size = match size.as_deref() {
        Some("small") | Some("list") => CoverSize::Small,
        Some("original") | Some("orig") => CoverSize::Original,
        _ => CoverSize::Mid,
    };

    let mut result = std::collections::HashMap::new();
    for hash in hashes {
        if let Some(url) = cache.get_cover_url(&hash, cover_size) {
            result.insert(hash, url);
        }
    }

    Ok(result)
}

/// Get cover cache statistics
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverCacheStats {
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub total_size_mb: f64,
}

#[tauri::command]
pub fn get_cover_cache_stats(
    cover_cache: State<'_, CoverCacheState>,
) -> Result<CoverCacheStats, String> {
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?;
    let stats = cache.get_stats();

    Ok(CoverCacheStats {
        file_count: stats.file_count,
        total_size_bytes: stats.total_size,
        total_size_mb: stats.total_size as f64 / 1024.0 / 1024.0,
    })
}

/// Clean up orphaned covers (not referenced by any song)
#[tauri::command]
pub fn cleanup_orphaned_covers(
    db: State<'_, DbState>,
    cover_cache: State<'_, CoverCacheState>,
) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?;

    // Get all cover hashes from DB
    let mut stmt = conn
        .prepare("SELECT DISTINCT cover_hash FROM songs WHERE cover_hash IS NOT NULL")
        .map_err(|e| e.to_string())?;

    let valid_hashes: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    cache.cleanup_orphaned(&valid_hashes)
}

/// Clear all cover cache
#[tauri::command]
pub fn clear_cover_cache(cover_cache: State<'_, CoverCacheState>) -> Result<usize, String> {
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?;
    cache.clear_all()
}

/// Clean up songs whose files no longer exist
#[tauri::command]
pub fn cleanup_missing_songs(db: State<'_, DbState>) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get all local songs
    let songs = db::songs::get_all_songs(&conn).map_err(|e| e.to_string())?;

    let missing_ids: Vec<String> = songs
        .iter()
        .filter(|s| s.source_type == "local" && !std::path::Path::new(&s.file_path).exists())
        .map(|s| s.id.clone())
        .collect();

    let count = missing_ids.len();

    for id in missing_ids {
        conn.execute("DELETE FROM songs WHERE id = ?1", [&id])
            .map_err(|e| e.to_string())?;
    }

    Ok(count)
}

// ============ Play Statistics Commands ============

/// Record a play for a song (increment play_count and update last_played_at)
#[tauri::command]
pub fn db_record_play(db: State<'_, DbState>, song_id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::record_play(&conn, &song_id).map_err(|e| e.to_string())
}

/// Get recently played songs
#[tauri::command]
pub fn db_get_recently_played(
    db: State<'_, DbState>,
    limit: Option<u32>,
) -> Result<Vec<DbSong>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::get_recently_played(&conn, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

/// Clear recently played history while preserving play counts.
#[tauri::command]
pub fn db_clear_recently_played(db: State<'_, DbState>) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::clear_recently_played(&conn).map_err(|e| e.to_string())
}

/// Get most played songs
#[tauri::command]
pub fn db_get_most_played(
    db: State<'_, DbState>,
    limit: Option<u32>,
) -> Result<Vec<DbSong>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::get_most_played(&conn, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

// ============ Home Recommendation Commands ============

/// Top played artist entry
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopArtist {
    pub name: String,
    pub song_count: i64,
    pub total_plays: i64,
    pub cover_hash: Option<String>,
    pub stream_cover_url: Option<String>,
}

/// Home page recommendation data
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeRecommendations {
    pub daily_mix: Vec<DbSong>,
    pub recently_played: Vec<DbSong>,
    pub newly_added: Vec<DbSong>,
    pub discovery: Vec<DbSong>,
    pub top_artists: Vec<TopArtist>,
    pub forgotten_favorites: Vec<DbSong>,
}

/// Get all home page recommendations in one call
#[tauri::command]
pub fn db_get_home_recommendations(db: State<'_, DbState>) -> Result<HomeRecommendations, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Use date-based seed for daily mix (same result all day)
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let seed: i64 = today
        .chars()
        .fold(0i64, |acc, c| acc.wrapping_mul(31).wrapping_add(c as i64));

    let daily_mix = db::songs::get_daily_mix(&conn, seed, 10).map_err(|e| e.to_string())?;
    let recently_played = db::songs::get_recently_played(&conn, 10).map_err(|e| e.to_string())?;
    let newly_added = db::songs::get_newly_added(&conn, 90, 10).map_err(|e| e.to_string())?;
    let discovery = db::songs::get_discovery_songs(&conn, 5, 10).map_err(|e| e.to_string())?;
    let forgotten_favorites =
        db::songs::get_forgotten_favorites(&conn, 14, 10).map_err(|e| e.to_string())?;

    let top_artist_tuples = db::songs::get_top_artists(&conn, 10).map_err(|e| e.to_string())?;
    let top_artists = top_artist_tuples
        .into_iter()
        .map(
            |(name, song_count, total_plays, cover_hash, stream_cover_url)| TopArtist {
                name,
                song_count,
                total_plays,
                cover_hash,
                stream_cover_url,
            },
        )
        .collect();

    Ok(HomeRecommendations {
        daily_mix,
        recently_played,
        newly_added,
        discovery,
        top_artists,
        forgotten_favorites,
    })
}

/// Get discovery songs (random low-play songs)
#[tauri::command]
pub fn db_get_discovery_songs(
    db: State<'_, DbState>,
    limit: Option<u32>,
) -> Result<Vec<DbSong>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::get_discovery_songs(&conn, 5, limit.unwrap_or(10)).map_err(|e| e.to_string())
}

// ============ File Watcher Commands ============

#[tauri::command]
pub fn start_file_watcher(
    #[allow(unused_variables)] app_handle: tauri::AppHandle,
    #[allow(unused_variables)] directories: Vec<String>,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        crate::watcher::desktop::start_watching(&app_handle, directories)
    }
    #[cfg(not(desktop))]
    {
        Ok(())
    }
}

#[tauri::command]
pub fn stop_file_watcher(
    #[allow(unused_variables)] app_handle: tauri::AppHandle,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        crate::watcher::desktop::stop_watching(&app_handle)
    }
    #[cfg(not(desktop))]
    {
        Ok(())
    }
}
