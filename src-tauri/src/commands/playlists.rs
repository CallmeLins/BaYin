//! Remote (stream) playlists commands.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{self, DbState};
use crate::models::{ServerType, StreamServerConfig};
use crate::utils::{jellyfin, subsonic};

const PLAYLISTS_TTL_SECS: i64 = 10 * 60;

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn server_type_from_str(s: &str) -> ServerType {
    match s {
        "navidrome" => ServerType::Navidrome,
        "subsonic" => ServerType::Subsonic,
        "opensubsonic" => ServerType::OpenSubsonic,
        "jellyfin" => ServerType::Jellyfin,
        "emby" => ServerType::Emby,
        _ => ServerType::Navidrome,
    }
}

fn build_config(server: &db::servers::DbStreamServer) -> StreamServerConfig {
    StreamServerConfig {
        server_type: server_type_from_str(server.server_type.as_str()),
        server_name: server.server_name.clone(),
        server_url: server.server_url.clone(),
        username: server.username.clone(),
        password: server.password.clone(),
        legacy_auth: server.legacy_auth,
        access_token: server.access_token.clone(),
        user_id: server.user_id.clone(),
    }
}

fn resolve_internal_ids(server_id: &str, song_ids: &[String]) -> Vec<String> {
    song_ids
        .iter()
        .map(|song_id| {
            if let Some(stripped) = song_id.strip_prefix(&format!("{}-", server_id)) {
                stripped.to_string()
            } else {
                song_id.clone()
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPlaylistSummary {
    pub server_id: String,
    pub server_name: String,
    pub server_type: String,
    pub playlist_id: String,
    pub name: String,
    pub song_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPlaylistTrack {
    pub id: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover_url: Option<String>,
}

fn summary_from_row(server: &db::servers::DbStreamServer, row: db::playlists::DbStreamPlaylist) -> StreamPlaylistSummary {
    StreamPlaylistSummary {
        server_id: server.id.clone(),
        server_name: server.server_name.clone(),
        server_type: server.server_type.clone(),
        playlist_id: row.playlist_id,
        name: row.name,
        song_count: row.song_count.max(0) as u32,
    }
}

async fn refresh_stream_playlists_cache(
    db: &State<'_, DbState>,
    server: &db::servers::DbStreamServer,
) -> Result<Vec<StreamPlaylistSummary>, String> {
    let config = build_config(server);
    let synced_at = now_ts();

    let fetched = if config.is_subsonic() {
        subsonic::fetch_playlists(&config)
            .await?
            .into_iter()
            .map(|(id, name, count, _changed)| (id, name, count))
            .collect::<Vec<_>>()
    } else {
        jellyfin::fetch_playlists(&config).await?
    };

    let db_rows: Vec<db::playlists::DbStreamPlaylist> = fetched
        .iter()
        .map(|(playlist_id, name, count)| db::playlists::DbStreamPlaylist {
            server_id: server.id.clone(),
            playlist_id: playlist_id.clone(),
            name: name.clone(),
            song_count: *count as i64,
            kind: "manual".to_string(),
            updated_at: None,
            synced_at,
        })
        .collect();

    {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        db::playlists::replace_stream_playlists(&mut conn, &server.id, &db_rows)
            .map_err(|e| e.to_string())?;
    }

    Ok(fetched
        .into_iter()
        .map(|(playlist_id, name, count)| StreamPlaylistSummary {
            server_id: server.id.clone(),
            server_name: server.server_name.clone(),
            server_type: server.server_type.clone(),
            playlist_id,
            name,
            song_count: count,
        })
        .collect())
}

async fn refresh_stream_playlist_cache(
    db: &State<'_, DbState>,
    server: &db::servers::DbStreamServer,
    playlist_id: &str,
) -> Result<(), String> {
    refresh_stream_playlists_cache(db, server).await?;

    let config = build_config(server);
    let remote_ids = if config.is_subsonic() {
        subsonic::fetch_playlist_song_ids(&config, playlist_id).await?
    } else {
        jellyfin::fetch_playlist_song_ids(&config, playlist_id).await?
    };

    let internal_ids: Vec<String> = remote_ids
        .into_iter()
        .map(|rid| format!("{}-{}", server.id, rid))
        .collect();

    let synced_at = now_ts();
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    db::playlists::replace_stream_playlist_items(
        &mut conn,
        &server.id,
        playlist_id,
        &internal_ids,
        synced_at,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn load_server(db: &State<'_, DbState>, server_id: &str) -> Result<db::servers::DbStreamServer, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::servers::get_stream_server_by_id(&conn, server_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Server not found".to_string())
}

#[tauri::command]
pub async fn stream_get_playlists(
    db: State<'_, DbState>,
    server_id: String,
    force_refresh: Option<bool>,
) -> Result<Vec<StreamPlaylistSummary>, String> {
    let force = force_refresh.unwrap_or(false);
    let now = now_ts();

    if !force {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if let Ok(Some(last_sync)) = db::playlists::get_stream_playlists_last_sync(&conn, &server_id) {
            if now.saturating_sub(last_sync) <= PLAYLISTS_TTL_SECS {
                let cached = db::playlists::get_stream_playlists(&conn, &server_id)
                    .map_err(|e| e.to_string())?;
                let server = db::servers::get_stream_server_by_id(&conn, &server_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "Server not found".to_string())?;
                return Ok(cached.into_iter().map(|row| summary_from_row(&server, row)).collect());
            }
        }
    }

    let server = load_server(&db, &server_id)?;
    refresh_stream_playlists_cache(&db, &server).await
}

#[tauri::command]
pub async fn stream_get_playlist_tracks(
    db: State<'_, DbState>,
    server_id: String,
    playlist_id: String,
    _force_refresh: Option<bool>,
) -> Result<Vec<StreamPlaylistTrack>, String> {
    let server = load_server(&db, &server_id)?;
    let config = build_config(&server);
    let remote_tracks = if config.is_subsonic() {
        subsonic::fetch_playlist_tracks(&config, &playlist_id).await?
    } else {
        jellyfin::fetch_playlist_tracks(&config, &playlist_id).await?
    };

    let tracks: Vec<StreamPlaylistTrack> = remote_tracks
        .into_iter()
        .map(|(rid, title, artist, album, cover_url)| StreamPlaylistTrack {
            id: format!("{}-{}", server_id, rid),
            title,
            artist,
            album,
            cover_url,
        })
        .collect();

    let internal_ids: Vec<String> = tracks.iter().map(|track| track.id.clone()).collect();

    let synced_at = now_ts();
    {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        db::playlists::replace_stream_playlist_items(
            &mut conn,
            &server_id,
            &playlist_id,
            &internal_ids,
            synced_at,
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(tracks)
}

#[tauri::command]
pub async fn stream_add_songs_to_playlist(
    db: State<'_, DbState>,
    server_id: String,
    playlist_id: String,
    song_ids: Vec<String>,
) -> Result<(), String> {
    if song_ids.is_empty() {
        return Ok(());
    }

    let server = load_server(&db, &server_id)?;
    let config = build_config(&server);
    let remote_song_ids = resolve_internal_ids(&server_id, &song_ids);

    if config.is_subsonic() {
        subsonic::add_songs_to_playlist(&config, &playlist_id, &remote_song_ids).await?;
    } else {
        jellyfin::add_songs_to_playlist(&config, &playlist_id, &remote_song_ids).await?;
    }

    refresh_stream_playlist_cache(&db, &server, &playlist_id).await
}

#[tauri::command]
pub async fn stream_create_playlist(
    db: State<'_, DbState>,
    server_id: String,
    name: String,
    song_ids: Vec<String>,
) -> Result<(), String> {
    let server = load_server(&db, &server_id)?;
    let config = build_config(&server);
    let remote_song_ids = resolve_internal_ids(&server_id, &song_ids);

    if config.is_subsonic() {
        subsonic::create_playlist(&config, &name, &remote_song_ids).await?;
    } else {
        jellyfin::create_playlist(&config, &name, &remote_song_ids).await?;
    }

    refresh_stream_playlists_cache(&db, &server).await.map(|_| ())
}

#[tauri::command]
pub async fn stream_rename_playlist(
    db: State<'_, DbState>,
    server_id: String,
    playlist_id: String,
    name: String,
) -> Result<(), String> {
    let server = load_server(&db, &server_id)?;
    let config = build_config(&server);

    if config.is_subsonic() {
        subsonic::rename_playlist(&config, &playlist_id, &name).await?;
    } else {
        jellyfin::rename_playlist(&config, &playlist_id, &name).await?;
    }

    refresh_stream_playlists_cache(&db, &server).await.map(|_| ())
}

#[tauri::command]
pub async fn stream_delete_playlist(
    db: State<'_, DbState>,
    server_id: String,
    playlist_id: String,
) -> Result<(), String> {
    let server = load_server(&db, &server_id)?;
    let config = build_config(&server);

    if config.is_subsonic() {
        subsonic::delete_playlist(&config, &playlist_id).await?;
    } else {
        jellyfin::delete_playlist(&config, &playlist_id).await?;
    }

    refresh_stream_playlists_cache(&db, &server).await.map(|_| ())
}

#[tauri::command]
pub async fn stream_remove_songs_from_playlist(
    db: State<'_, DbState>,
    server_id: String,
    playlist_id: String,
    song_ids: Vec<String>,
) -> Result<(), String> {
    if song_ids.is_empty() {
        return Ok(());
    }

    let server = load_server(&db, &server_id)?;
    let config = build_config(&server);
    let remote_song_ids = resolve_internal_ids(&server_id, &song_ids);

    if config.is_subsonic() {
        let current_remote_ids = subsonic::fetch_playlist_song_ids(&config, &playlist_id).await?;
        let indexes: Vec<usize> = current_remote_ids
            .iter()
            .enumerate()
            .filter_map(|(index, remote_id)| remote_song_ids.iter().any(|id| id == remote_id).then_some(index))
            .collect();
        subsonic::remove_playlist_indexes(&config, &playlist_id, &indexes).await?;
    } else {
        jellyfin::remove_songs_from_playlist(&config, &playlist_id, &remote_song_ids).await?;
    }

    refresh_stream_playlist_cache(&db, &server, &playlist_id).await
}
