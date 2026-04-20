use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::db::{
    self, DbAlbum, DbArtist, DbSong, DbStreamServer, ScanConfig, SongInput, StreamServerInput,
};
use crate::db::playlists::{get_stream_playlists, DbStreamPlaylist};
use crate::state::{init_database, with_db, with_db_mut};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub total_songs: i64,
    pub local_songs: i64,
    pub stream_songs: i64,
    pub total_albums: i64,
    pub total_artists: i64,
}

pub fn init_db(db_path: String) -> Result<(), String> {
    init_database(Path::new(&db_path))
}

pub fn db_get_all_songs() -> Result<Vec<DbSong>, String> {
    with_db(|conn| db::songs::get_all_songs(conn).map_err(|e| e.to_string()))
}

pub fn db_get_all_albums() -> Result<Vec<DbAlbum>, String> {
    with_db(|conn| db::albums::get_all_albums(conn).map_err(|e| e.to_string()))
}

pub fn db_get_all_artists() -> Result<Vec<DbArtist>, String> {
    with_db(|conn| db::albums::get_all_artists(conn).map_err(|e| e.to_string()))
}

pub fn db_save_songs(
    songs: Vec<SongInput>,
    source_type: String,
    server_id: Option<String>,
) -> Result<u64, String> {
    with_db_mut(|conn| {
        db::songs::save_songs(conn, &songs, &source_type, server_id.as_deref())
            .map(|count| count as u64)
            .map_err(|e| e.to_string())
    })
}

pub fn db_delete_songs_by_source(
    source_type: String,
    server_id: Option<String>,
) -> Result<u64, String> {
    with_db(|conn| {
        db::songs::delete_songs_by_source(conn, &source_type, server_id.as_deref())
            .map(|count| count as u64)
            .map_err(|e| e.to_string())
    })
}

pub fn db_clear_all_songs() -> Result<u64, String> {
    with_db(|conn| {
        db::songs::clear_all_songs(conn)
            .map(|count| count as u64)
            .map_err(|e| e.to_string())
    })
}

pub fn db_get_stream_servers() -> Result<Vec<DbStreamServer>, String> {
    with_db(|conn| db::servers::get_stream_servers(conn).map_err(|e| e.to_string()))
}

pub fn db_get_stream_playlists(server_id: String) -> Result<Vec<DbStreamPlaylist>, String> {
    with_db(|conn| get_stream_playlists(conn, &server_id).map_err(|e| e.to_string()))
}

pub fn db_save_stream_server(config: StreamServerInput) -> Result<String, String> {
    with_db(|conn| db::servers::save_stream_server(conn, &config).map_err(|e| e.to_string()))
}

pub fn db_delete_stream_server(server_id: String) -> Result<(), String> {
    with_db(|conn| db::servers::delete_stream_server(conn, &server_id).map_err(|e| e.to_string()))
}

pub fn db_clear_stream_servers() -> Result<(), String> {
    with_db(|conn| db::servers::clear_stream_servers(conn).map_err(|e| e.to_string()))
}

pub fn db_save_scan_config(config: ScanConfig) -> Result<(), String> {
    with_db(|conn| db::servers::save_scan_config(conn, &config).map_err(|e| e.to_string()))
}

pub fn db_get_scan_config() -> Result<Option<ScanConfig>, String> {
    with_db(|conn| db::servers::get_scan_config(conn).map_err(|e| e.to_string()))
}

pub fn db_clear_scan_config() -> Result<(), String> {
    with_db(|conn| db::servers::clear_scan_config(conn).map_err(|e| e.to_string()))
}

pub fn db_migrate_from_localstorage(data: MigrationData) -> Result<u64, String> {
    with_db_mut(|conn| {
        let existing_count = db::songs::get_song_count(conn).map_err(|e| e.to_string())?;
        if existing_count > 0 {
            return Ok(0);
        }

        let mut local_songs = Vec::new();
        let mut stream_songs = Vec::new();

        for song in data.songs {
            let file_path = song.file_path.unwrap_or_default();
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

        let mut total = 0u64;

        if !local_songs.is_empty() {
            total += db::songs::save_songs(conn, &local_songs, "local", None)
                .map_err(|e| e.to_string())? as u64;
        }

        let server_id = if let Some(config) = data.stream_config {
            let input = StreamServerInput {
                server_type: config.server_type,
                server_name: config.server_name,
                server_url: config.server_url,
                username: config.username,
                password: config.password,
                access_token: config.access_token,
                user_id: config.user_id,
            };
            Some(db::servers::save_stream_server(conn, &input).map_err(|e| e.to_string())?)
        } else {
            None
        };

        if !stream_songs.is_empty() {
            total += db::songs::save_songs(conn, &stream_songs, "stream", server_id.as_deref())
                .map_err(|e| e.to_string())? as u64;
        }

        Ok(total)
    })
}

pub fn db_get_library_stats() -> Result<LibraryStats, String> {
    with_db(|conn| {
        let total_songs = db::songs::get_song_count(conn).map_err(|e| e.to_string())?;
        let local_songs =
            db::songs::get_song_count_by_source(conn, "local").map_err(|e| e.to_string())?;
        let stream_songs =
            db::songs::get_song_count_by_source(conn, "stream").map_err(|e| e.to_string())?;

        let albums = db::albums::get_all_albums(conn).map_err(|e| e.to_string())?;
        let artists = db::albums::get_all_artists(conn).map_err(|e| e.to_string())?;

        Ok(LibraryStats {
            total_songs,
            local_songs,
            stream_songs,
            total_albums: albums.len() as i64,
            total_artists: artists.len() as i64,
        })
    })
}
