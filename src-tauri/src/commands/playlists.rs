//! Remote (stream) playlists commands.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{self, DbState};
use crate::models::{ServerType, StreamServerConfig};
use crate::utils::{jellyfin, subsonic};

const PLAYLISTS_TTL_SECS: i64 = 10 * 60; // 10 minutes
const PLAYLIST_ITEMS_TTL_SECS: i64 = 30 * 60; // 30 minutes

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
        access_token: server.access_token.clone(),
        user_id: server.user_id.clone(),
    }
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

#[tauri::command]
pub async fn stream_get_playlists(
    db: State<'_, DbState>,
    server_id: String,
    force_refresh: Option<bool>,
) -> Result<Vec<StreamPlaylistSummary>, String> {
    let force = force_refresh.unwrap_or(false);
    let now = now_ts();

    // Try cache first.
    if !force {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if let Ok(Some(last_sync)) = db::playlists::get_stream_playlists_last_sync(&conn, &server_id) {
            if now.saturating_sub(last_sync) <= PLAYLISTS_TTL_SECS {
                let cached = db::playlists::get_stream_playlists(&conn, &server_id)
                    .map_err(|e| e.to_string())?;
                // server metadata for grouping
                let server = db::servers::get_stream_server_by_id(&conn, &server_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "服务器不存在".to_string())?;
                return Ok(cached
                    .into_iter()
                    .map(|p| StreamPlaylistSummary {
                        server_id: server.id.clone(),
                        server_name: server.server_name.clone(),
                        server_type: server.server_type.clone(),
                        playlist_id: p.playlist_id,
                        name: p.name,
                        song_count: p.song_count.max(0) as u32,
                    })
                    .collect());
            }
        }
    }

    // Fetch remote.
    let server = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::servers::get_stream_server_by_id(&conn, &server_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "服务器不存在".to_string())?
    };
    let config = build_config(&server);

    let synced_at = now_ts();
    let fetched = if config.is_subsonic() {
        // (id, name, song_count, changed)
        subsonic::fetch_playlists(&config).await?
            .into_iter()
            .map(|(id, name, count, _changed)| (id, name, count))
            .collect::<Vec<_>>()
    } else {
        jellyfin::fetch_playlists(&config).await?
    };

    let db_rows: Vec<db::playlists::DbStreamPlaylist> = fetched
        .iter()
        .map(|(pid, name, count)| db::playlists::DbStreamPlaylist {
            server_id: server_id.clone(),
            playlist_id: pid.clone(),
            name: name.clone(),
            song_count: *count as i64,
            kind: "manual".to_string(),
            updated_at: None,
            synced_at,
        })
        .collect();

    {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        db::playlists::replace_stream_playlists(&mut conn, &server_id, &db_rows)
            .map_err(|e| e.to_string())?;
    }

    Ok(fetched
        .into_iter()
        .map(|(pid, name, count)| StreamPlaylistSummary {
            server_id: server.id.clone(),
            server_name: server.server_name.clone(),
            server_type: server.server_type.clone(),
            playlist_id: pid,
            name,
            song_count: count,
        })
        .collect())
}

#[tauri::command]
pub async fn stream_get_playlist_tracks(
    db: State<'_, DbState>,
    server_id: String,
    playlist_id: String,
    force_refresh: Option<bool>,
) -> Result<Vec<String>, String> {
    let force = force_refresh.unwrap_or(false);
    let now = now_ts();

    if !force {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if let Ok(Some(last_sync)) =
            db::playlists::get_stream_playlist_items_last_sync(&conn, &server_id, &playlist_id)
        {
            if now.saturating_sub(last_sync) <= PLAYLIST_ITEMS_TTL_SECS {
                let cached = db::playlists::get_stream_playlist_items(&conn, &server_id, &playlist_id)
                    .map_err(|e| e.to_string())?;
                return Ok(cached);
            }
        }
    }

    let server = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::servers::get_stream_server_by_id(&conn, &server_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "服务器不存在".to_string())?
    };
    let config = build_config(&server);

    let remote_ids: Vec<String> = if config.is_subsonic() {
        subsonic::fetch_playlist_song_ids(&config, &playlist_id).await?
    } else {
        jellyfin::fetch_playlist_song_ids(&config, &playlist_id).await?
    };

    // Normalize into the internal song id format used by our stream scan.
    let internal_ids: Vec<String> = remote_ids
        .into_iter()
        .map(|rid| format!("{}-{}", server_id, rid))
        .collect();

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

    Ok(internal_ids)
}

