use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

use crate::db::{self, DbSong, DbStreamServer, StreamServerInput};
use crate::models::{ConnectionTestResult, ServerType, StreamServerConfig};
use crate::state::{with_db, with_db_mut};
use crate::utils::{jellyfin, subsonic};

static TOKIO_RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Failed to create tokio runtime for streaming API"));

fn block_on<F: Future>(future: F) -> F::Output {
    TOKIO_RUNTIME.block_on(future)
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn parse_server_type(value: &str) -> Result<ServerType, String> {
    match value.to_ascii_lowercase().as_str() {
        "navidrome" => Ok(ServerType::Navidrome),
        "subsonic" => Ok(ServerType::Subsonic),
        "opensubsonic" => Ok(ServerType::OpenSubsonic),
        "jellyfin" => Ok(ServerType::Jellyfin),
        "emby" => Ok(ServerType::Emby),
        other => Err(format!("Unsupported stream server type: {other}")),
    }
}

fn server_type_to_string(value: &ServerType) -> String {
    match value {
        ServerType::Navidrome => "navidrome".to_string(),
        ServerType::Subsonic => "subsonic".to_string(),
        ServerType::OpenSubsonic => "opensubsonic".to_string(),
        ServerType::Jellyfin => "jellyfin".to_string(),
        ServerType::Emby => "emby".to_string(),
    }
}

fn build_config(server: &DbStreamServer) -> Result<StreamServerConfig, String> {
    Ok(StreamServerConfig {
        server_type: parse_server_type(&server.server_type)?,
        server_name: server.server_name.clone(),
        server_url: server.server_url.clone(),
        username: server.username.clone(),
        password: server.password.clone(),
        access_token: server.access_token.clone(),
        user_id: server.user_id.clone(),
    })
}

fn save_config(config: &StreamServerConfig) -> Result<(), String> {
    crate::api::db::db_save_stream_server(StreamServerInput {
        server_type: server_type_to_string(&config.server_type),
        server_name: config.server_name.clone(),
        server_url: config.server_url.clone(),
        username: config.username.clone(),
        password: config.password.clone(),
        access_token: config.access_token.clone(),
        user_id: config.user_id.clone(),
    })
    .map(|_| ())
}

fn ensure_jellyfin_credentials(config: &mut StreamServerConfig) -> Result<(), String> {
    if !config.is_jellyfin_like() {
        return Ok(());
    }
    if config.access_token.is_some() && config.user_id.is_some() {
        return Ok(());
    }
    let (access_token, user_id) = block_on(jellyfin::authenticate(config))?;
    config.access_token = Some(access_token);
    config.user_id = Some(user_id);
    save_config(config)
}

fn load_server(server_id: &str) -> Result<DbStreamServer, String> {
    with_db(|conn| {
        db::servers::get_stream_server_by_id(conn, server_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Stream server not found: {server_id}"))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamConnectionTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPlaylistSyncResult {
    pub server_id: String,
    pub server_name: String,
    pub playlist_count: usize,
    pub synced_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPlaylistSongsRequest {
    pub server_id: String,
    pub playlist_id: String,
}

#[derive(Debug, Clone)]
struct RemotePlaylistSummary {
    playlist_id: String,
    name: String,
    song_count: u32,
    updated_at: Option<i64>,
}

fn to_test_result(value: ConnectionTestResult) -> StreamConnectionTestResult {
    StreamConnectionTestResult {
        success: value.success,
        message: value.message,
        server_version: value.server_version,
        access_token: None,
        user_id: None,
    }
}

pub fn test_stream_connection(
    input: StreamServerInput,
) -> Result<StreamConnectionTestResult, String> {
    let config = StreamServerConfig {
        server_type: parse_server_type(&input.server_type)?,
        server_name: input.server_name,
        server_url: input.server_url,
        username: input.username,
        password: input.password,
        access_token: input.access_token,
        user_id: input.user_id,
    };

    let mut result = if config.is_subsonic() {
        to_test_result(block_on(subsonic::test_connection(&config)))
    } else {
        to_test_result(block_on(jellyfin::test_connection(&config)))
    };

    if result.success && config.is_jellyfin_like() {
        let (access_token, user_id) = block_on(jellyfin::authenticate(&config))?;
        result.access_token = Some(access_token);
        result.user_id = Some(user_id);
    }

    Ok(result)
}

pub fn sync_stream_playlists(server_id: String) -> Result<StreamPlaylistSyncResult, String> {
    let server = load_server(&server_id)?;
    let mut config = build_config(&server)?;
    ensure_jellyfin_credentials(&mut config)?;

    let remote_playlists: Vec<RemotePlaylistSummary> = if config.is_subsonic() {
        block_on(subsonic::fetch_playlists(&config))?
            .into_iter()
            .map(
                |(playlist_id, name, song_count, _changed)| RemotePlaylistSummary {
                    playlist_id,
                    name,
                    song_count,
                    updated_at: None,
                },
            )
            .collect()
    } else {
        block_on(jellyfin::fetch_playlists(&config))?
            .into_iter()
            .map(|(playlist_id, name, song_count)| RemotePlaylistSummary {
                playlist_id,
                name,
                song_count,
                updated_at: None,
            })
            .collect()
    };

    let synced_at = now_ts();
    let db_rows = remote_playlists
        .iter()
        .map(|playlist| db::playlists::DbStreamPlaylist {
            server_id: server_id.clone(),
            playlist_id: playlist.playlist_id.clone(),
            name: playlist.name.clone(),
            song_count: playlist.song_count as i64,
            kind: "manual".to_string(),
            updated_at: playlist.updated_at,
            synced_at,
        })
        .collect::<Vec<_>>();

    with_db_mut(|conn| {
        db::playlists::replace_stream_playlists(conn, &server_id, &db_rows)
            .map_err(|e| e.to_string())
    })?;

    Ok(StreamPlaylistSyncResult {
        server_id,
        server_name: server.server_name,
        playlist_count: remote_playlists.len(),
        synced_at,
    })
}

pub fn get_stream_playlist_songs(
    request: StreamPlaylistSongsRequest,
) -> Result<Vec<DbSong>, String> {
    let server = load_server(&request.server_id)?;
    let mut config = build_config(&server)?;
    ensure_jellyfin_credentials(&mut config)?;

    let remote_tracks = if config.is_subsonic() {
        block_on(subsonic::fetch_playlist_tracks(
            &config,
            &request.playlist_id,
        ))?
    } else {
        block_on(jellyfin::fetch_playlist_tracks(
            &config,
            &request.playlist_id,
        ))?
    };

    let now = now_ts();
    let request_server_id = request.server_id.clone();
    let request_playlist_id = request.playlist_id.clone();
    let server_type = server.server_type.clone();
    let server_name = server.server_name.clone();
    let songs = remote_tracks
        .into_iter()
        .enumerate()
        .map(|(index, (remote_id, title, artist, album, cover_url))| {
            let stream_url = if config.is_subsonic() {
                subsonic::get_stream_url(&config, &remote_id)
            } else {
                jellyfin::get_stream_url(&config, &remote_id)
            };
            let internal_id = format!("{}-{}", request_server_id, remote_id);
            let stream_info = serde_json::json!({
                "type": "stream",
                "serverId": request_server_id,
                "serverType": server_type,
                "serverName": server_name,
                "playlistId": request_playlist_id,
                "songId": remote_id,
                "streamUrl": stream_url,
                "coverUrl": cover_url,
            })
            .to_string();

            DbSong {
                id: internal_id,
                title: title.unwrap_or_else(|| format!("Track {}", index + 1)),
                artist: artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                album: album.unwrap_or_else(|| "Unknown Album".to_string()),
                duration: 0.0,
                file_path: String::new(),
                file_size: 0,
                is_hr: None,
                is_sq: None,
                cover_hash: None,
                source_type: "stream".to_string(),
                server_id: Some(request_server_id.clone()),
                server_song_id: Some(remote_id),
                stream_info: Some(stream_info),
                file_modified: None,
                format: None,
                bit_depth: None,
                sample_rate: None,
                bitrate: None,
                channels: None,
                created_at: now,
                updated_at: now,
            }
        })
        .collect::<Vec<_>>();

    let internal_ids = songs.iter().map(|song| song.id.clone()).collect::<Vec<_>>();
    with_db_mut(|conn| {
        db::playlists::replace_stream_playlist_items(
            conn,
            &request.server_id,
            &request.playlist_id,
            &internal_ids,
            now,
        )
        .map_err(|e| e.to_string())
    })?;

    Ok(songs)
}
