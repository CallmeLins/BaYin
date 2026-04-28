//! Subsonic API 工具函数
//! 支持 Navidrome、Subsonic、OpenSubsonic 等兼容服务器
use std::collections::HashSet;

use rand::Rng;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

use crate::models::{
    ConnectionTestResult, GetAlbumList2Response, GetArtistResponse, GetArtistsResponse,
    PingResponse, ScannedSong, StreamServerConfig,
    SubsonicPlaylistResponse, SubsonicPlaylistsResponse, SubsonicResponse, SubsonicSong,
};
use crate::utils::audio::extract_filename_from_path_str;
use crate::utils::datetime::parse_datetime_to_epoch_seconds;
use crate::utils::http::build_client;

/// Lossless audio suffixes.
const LOSSLESS_SUFFIXES: &[&str] = &["flac", "wav", "ape", "aiff", "dsf", "dff", "alac"];
const SUBSONIC_API_VERSION: &str = "1.16.1";
const SUBSONIC_LEGACY_API_VERSION: &str = "1.12.0";
const SEARCH_PAGE_SIZE: u64 = 5000;
const ALBUM_PAGE_SIZE: u64 = 500;

/// 生成 Subsonic API 认证参数
fn generate_auth_params(config: &StreamServerConfig, include_format: bool) -> Vec<(&str, String)> {
    let version = if config.legacy_auth {
        SUBSONIC_LEGACY_API_VERSION
    } else {
        SUBSONIC_API_VERSION
    };

    if config.legacy_auth {
        let mut params = vec![
            ("u", config.username.clone()),
            ("p", config.password.clone()),
            ("v", version.to_string()),
            ("c", "BaYin".to_string()),
        ];
        if include_format {
            params.push(("f", "json".to_string()));
        }
        return params;
    }

    let salt: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    let token = format!("{:x}", md5::compute(format!("{}{}", config.password, salt)));

    let mut params = vec![
        ("u", config.username.clone()),
        ("t", token),
        ("s", salt),
        ("v", version.to_string()),
        ("c", "BaYin".to_string()),
    ];
    if include_format {
        params.push(("f", "json".to_string()));
    }
    params
}

/// 鏋勫缓 API URL
fn build_url(config: &StreamServerConfig, endpoint: &str) -> String {
    let base = config.server_url.trim_end_matches('/');
    format!("{}/rest/{}", base, endpoint)
}

fn build_cover_art_url(config: &StreamServerConfig, cover_id: &str) -> String {
    let base = config.server_url.trim_end_matches('/');
    let params = generate_auth_params(config, false);
    let query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}/rest/getCoverArt?id={}&{}", base, cover_id, query)
}

fn music_folder_param(config: &StreamServerConfig) -> Option<(&'static str, String)> {
    let id = config.music_folder_id.as_ref()?.trim();
    if id.is_empty() {
        None
    } else {
        Some(("musicFolderId", id.to_string()))
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn truncate_for_log(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let truncated: String = input.chars().take(max_chars).collect();
    format!("{}…(truncated)", truncated)
}

fn summarize_subsonic_payload(endpoint: &str, sub: &serde_json::Map<String, Value>) -> String {
    let mut details: Vec<String> = vec![format!("keys={}", sub.keys().cloned().collect::<Vec<_>>().join(","))];

    match endpoint {
        "search3" => {
            let shape = sub
                .get("searchResult3")
                .map(json_kind)
                .unwrap_or("missing");
            let song_shape = sub
                .get("searchResult3")
                .and_then(|v| v.get("song"))
                .map(json_kind)
                .unwrap_or("missing");
            details.push(format!("searchResult3={}", shape));
            details.push(format!("searchResult3.song={}", song_shape));
        }
        "getAlbumList2" => {
            let list_shape = sub.get("albumList2").map(json_kind).unwrap_or("missing");
            let album_shape = sub
                .get("albumList2")
                .and_then(|v| v.get("album"))
                .map(json_kind)
                .unwrap_or("missing");
            details.push(format!("albumList2={}", list_shape));
            details.push(format!("albumList2.album={}", album_shape));
        }
        "getAlbum" => {
            let album_shape = sub.get("album").map(json_kind).unwrap_or("missing");
            let song_shape = sub
                .get("album")
                .and_then(|v| v.get("song"))
                .map(json_kind)
                .unwrap_or("missing");
            details.push(format!("album={}", album_shape));
            details.push(format!("album.song={}", song_shape));
        }
        "getArtists" => {
            let artists_shape = sub.get("artists").map(json_kind).unwrap_or("missing");
            let index_shape = sub
                .get("artists")
                .and_then(|v| v.get("index"))
                .map(json_kind)
                .unwrap_or("missing");
            details.push(format!("artists={}", artists_shape));
            details.push(format!("artists.index={}", index_shape));
        }
        "getArtist" => {
            let artist_shape = sub.get("artist").map(json_kind).unwrap_or("missing");
            let album_shape = sub
                .get("artist")
                .and_then(|v| v.get("album"))
                .map(json_kind)
                .unwrap_or("missing");
            details.push(format!("artist={}", artist_shape));
            details.push(format!("artist.album={}", album_shape));
        }
        _ => {}
    }

    details.join(" | ")
}

/// 测试连接
pub async fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => return ConnectionTestResult { success: false, message: e, server_version: None },
    };
    let url = build_url(config, "ping");
    let params = generate_auth_params(config, true);

    match client.get(&url).query(&params).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                return ConnectionTestResult {
                    success: false,
                    message: format!("服务器返回错误: {}", response.status()),
                    server_version: None,
                };
            }

            match response.json::<SubsonicResponse<PingResponse>>().await {
                Ok(data) => {
                    let inner = data.subsonic_response;
                    if inner.status == "ok" {
                        ConnectionTestResult {
                            success: true,
                            message: "连接成功".to_string(),
                            server_version: Some(inner.version),
                        }
                    } else if let Some(error) = inner.error {
                        ConnectionTestResult {
                            success: false,
                            message: format!("认证失败: {}", error.message),
                            server_version: None,
                        }
                    } else {
                        ConnectionTestResult {
                            success: false,
                            message: "未知错误".to_string(),
                            server_version: None,
                        }
                    }
                }
                Err(e) => ConnectionTestResult {
                    success: false,
                    message: format!("解析响应失败: {}", e),
                    server_version: None,
                },
            }
        }
        Err(e) => ConnectionTestResult {
            success: false,
            message: format!("连接失败: {}", e),
            server_version: None,
        },
    }
}

/// 转换 Subsonic 歌曲为 ScannedSong
fn convert_song(song: &SubsonicSong, config: &StreamServerConfig) -> ScannedSong {
    let suffix = song.suffix.as_deref().unwrap_or("");
    let is_sq = LOSSLESS_SUFFIXES.contains(&suffix.to_lowercase().as_str());
    let is_hr = song.sampling_rate.map(|r| r > 44100).unwrap_or(false)
        || song.bit_depth.map(|d| d > 16).unwrap_or(false);

    // 构建封面 URL
    let cover_url = song
        .cover_art
        .as_ref()
        .map(|cover_id| build_cover_art_url(config, cover_id));

    // 标题为空时尝试从路径提取文件名
    let title = if song.title.is_empty() {
        song.path
            .as_ref()
            .and_then(|p| extract_filename_from_path_str(p))
            .unwrap_or_else(|| song.id.clone())
    } else {
        song.title.clone()
    };

    ScannedSong {
        id: song.id.clone(),
        title,
        artist: song
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string()),
        album: song
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string()),
        duration: song.duration.unwrap_or(0) as f64,
        file_path: song.path.clone().unwrap_or_default(),
        file_size: song.size.unwrap_or(0),
        cover_url,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
        format: song.suffix.as_ref().map(|s| s.to_uppercase()),
        bit_depth: song.bit_depth.and_then(|value| u8::try_from(value).ok()),
        sample_rate: song.sampling_rate,
        bitrate: song.bit_rate,
        channels: None,
        created_at: song.created.as_deref().and_then(parse_datetime_to_epoch_seconds),
    }
}

/// Scan all songs from a Subsonic-compatible server.
async fn request_subsonic_value(
    client: &Client,
    config: &StreamServerConfig,
    endpoint: &str,
    extra_params: Vec<(&str, String)>,
) -> Result<Value, String> {
    let url = build_url(config, endpoint);
    let mut params = generate_auth_params(config, true);
    params.extend(extra_params);

    let response = client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("请求失败: HTTP {}", response.status()));
    }

    let root: Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let sub = root
        .get("subsonic-response")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "解析响应失败: 缺少 subsonic-response".to_string())?;

    let status = sub
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if status != "ok" {
        let code = sub
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let msg = sub
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        log::debug!(
            "[subsonic-debug] endpoint={} auth={} musicFolderId={} api_error_code={} api_error_msg={}",
            endpoint,
            if config.legacy_auth { "legacy" } else { "token" },
            config.music_folder_id.as_deref().unwrap_or("<none>"),
            code,
            msg
        );
        return Err(format!("API 错误: {}", msg));
    }

    Ok(Value::Object(sub.clone()))
}

async fn request_subsonic<T: DeserializeOwned>(
    client: &Client,
    config: &StreamServerConfig,
    endpoint: &str,
    extra_params: Vec<(&str, String)>,
) -> Result<T, String> {
    let sub_value = request_subsonic_value(client, config, endpoint, extra_params).await?;
    serde_json::from_value(sub_value.clone()).map_err(|e| {
        let summary = sub_value
            .as_object()
            .map(|sub| summarize_subsonic_payload(endpoint, sub))
            .unwrap_or_else(|| "subsonic-response is not object".to_string());
        let snippet = truncate_for_log(&sub_value.to_string(), 1400);
        log::debug!(
            "[subsonic-debug] endpoint={} auth={} musicFolderId={} parse_error={} summary={}",
            endpoint,
            if config.legacy_auth { "legacy" } else { "token" },
            config.music_folder_id.as_deref().unwrap_or("<none>"),
            e,
            summary
        );
        log::debug!(
            "[subsonic-debug] endpoint={} payload_snippet={}",
            endpoint, snippet
        );
        format!("API 响应字段解析失败({}): {}", endpoint, e)
    })
}

fn looks_like_song_object(map: &serde_json::Map<String, Value>) -> bool {
    if !map.contains_key("id") {
        return false;
    }
    map.contains_key("title")
        || map.contains_key("artist")
        || map.contains_key("album")
        || map.contains_key("duration")
        || map.contains_key("suffix")
        || map.contains_key("path")
        || map.contains_key("contentType")
        || map.contains_key("bitRate")
        || map.contains_key("samplingRate")
        || map.contains_key("coverArt")
        || map.contains_key("track")
}

fn collect_songs_from_value(value: &Value, out: &mut Vec<SubsonicSong>, depth: usize) {
    if depth > 4 {
        return;
    }

    match value {
        Value::Array(items) => {
            for item in items {
                collect_songs_from_value(item, out, depth + 1);
            }
        }
        Value::Object(map) => {
            if looks_like_song_object(map) {
                if let Ok(song) = serde_json::from_value::<SubsonicSong>(Value::Object(map.clone())) {
                    out.push(song);
                    return;
                }
            }

            for key in ["song", "songs", "entry", "child", "track", "items", "data"] {
                if let Some(next) = map.get(key) {
                    collect_songs_from_value(next, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

fn extract_search3_songs(sub: &Value) -> Vec<SubsonicSong> {
    let mut songs = Vec::new();
    if let Some(search_result) = sub.get("searchResult3") {
        if let Some(song_value) = search_result.get("song") {
            collect_songs_from_value(song_value, &mut songs, 0);
        } else {
            collect_songs_from_value(search_result, &mut songs, 0);
        }
    }
    songs
}

fn extract_album_songs(sub: &Value) -> Vec<SubsonicSong> {
    let mut songs = Vec::new();
    if let Some(album_value) = sub.get("album") {
        if let Some(song_value) = album_value.get("song") {
            collect_songs_from_value(song_value, &mut songs, 0);
        } else {
            collect_songs_from_value(album_value, &mut songs, 0);
        }
    }
    songs
}

async fn fetch_songs_via_search3(
    client: &Client,
    config: &StreamServerConfig,
) -> Result<Vec<SubsonicSong>, String> {
    let mut songs = Vec::new();
    let mut offset = 0_u64;

    loop {
        let mut params = vec![
            ("query", "".to_string()),
            ("songCount", SEARCH_PAGE_SIZE.to_string()),
            ("songOffset", offset.to_string()),
            ("albumCount", "0".to_string()),
            ("albumOffset", "0".to_string()),
            ("artistCount", "0".to_string()),
            ("artistOffset", "0".to_string()),
        ];
        if let Some(param) = music_folder_param(config) {
            params.push(param);
        }

        let sub = request_subsonic_value(
            client,
            config,
            "search3",
            params,
        )
        .await?;

        let page_songs = extract_search3_songs(&sub);
        let count = page_songs.len() as u64;
        if count == 0 {
            log::debug!(
                "[subsonic-debug] endpoint=search3 empty-page offset={} payload_snippet={}",
                offset,
                truncate_for_log(&sub.to_string(), 1000)
            );
        }

        songs.extend(page_songs);
        offset += count;

        if count < SEARCH_PAGE_SIZE {
            break;
        }
    }

    Ok(songs)
}

async fn fetch_album_ids_via_album_list2(
    client: &Client,
    config: &StreamServerConfig,
) -> Result<Vec<String>, String> {
    let mut album_ids = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0_u64;

    loop {
        let mut params = vec![
            ("type", "alphabeticalByName".to_string()),
            ("size", ALBUM_PAGE_SIZE.to_string()),
            ("offset", offset.to_string()),
        ];
        if let Some(param) = music_folder_param(config) {
            params.push(param);
        }

        let data: GetAlbumList2Response = request_subsonic(
            client,
            config,
            "getAlbumList2",
            params,
        )
        .await?;

        let albums = data
            .album_list2
            .and_then(|list| list.album)
            .map(|album| album.into_vec())
            .unwrap_or_default();
        let count = albums.len() as u64;

        for album in albums {
            if seen.insert(album.id.clone()) {
                album_ids.push(album.id);
            }
        }

        offset += count;
        if count < ALBUM_PAGE_SIZE {
            break;
        }
    }

    Ok(album_ids)
}

async fn fetch_artist_ids(
    client: &Client,
    config: &StreamServerConfig,
) -> Result<Vec<String>, String> {
    let mut params = Vec::new();
    if let Some(param) = music_folder_param(config) {
        params.push(param);
    }
    let data: GetArtistsResponse = request_subsonic(client, config, "getArtists", params).await?;

    let mut artist_ids = Vec::new();
    let mut seen = HashSet::new();
    let indexes = data
        .artists
        .and_then(|artists| artists.index)
        .map(|index| index.into_vec())
        .unwrap_or_default();

    for index in indexes {
        let artists = index.artist.map(|artist| artist.into_vec()).unwrap_or_default();
        for artist in artists {
            if seen.insert(artist.id.clone()) {
                artist_ids.push(artist.id);
            }
        }
    }

    Ok(artist_ids)
}

async fn fetch_album_ids_via_artists(
    _client: &Client,
    config: &StreamServerConfig,
) -> Result<Vec<String>, String> {
    let artist_ids = fetch_artist_ids(_client, config).await?;
    if artist_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Parallelize per-artist requests to avoid N+1 sequential bottleneck.
    let results: Vec<Result<Vec<String>, String>> = artist_ids
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(16) // up to 16 concurrent requests
        .flat_map(|chunk| {
            use rayon::prelude::*;
            let bclient = reqwest::blocking::Client::new();
            chunk
                .par_iter()
                .map(|artist_id| {
                    let url = build_url(config, "getArtist");
                    let mut params = generate_auth_params(config, true);
                    params.push(("id", artist_id.to_string()));
                    let resp = bclient
                        .get(&url)
                        .query(&params)
                        .send()
                        .map_err(|e| format!("Request failed: {}", e))?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status()));
                    }
                    let root: Value = resp
                        .json()
                        .map_err(|e| format!("Parse error: {}", e))?;
                    let sub = root
                        .get("subsonic-response")
                        .and_then(|v| v.as_object())
                        .ok_or("Missing subsonic-response")?;
                    let data: GetArtistResponse =
                        serde_json::from_value(Value::Object(sub.clone()))
                            .map_err(|e| format!("Deserialize error: {}", e))?;
                    let albums = data
                        .artist
                        .and_then(|artist| artist.album)
                        .map(|album| album.into_vec())
                        .unwrap_or_default();
                    Ok(albums.into_iter().map(|a| a.id).collect::<Vec<_>>())
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let mut album_ids = Vec::new();
    let mut seen = HashSet::new();
    for result in results {
        match result {
            Ok(ids) => {
                for id in ids {
                    if seen.insert(id.clone()) {
                        album_ids.push(id);
                    }
                }
            }
            Err(err) => {
                log::warn!("Subsonic artist fallback failed: {}", err);
            }
        }
    }

    Ok(album_ids)
}

async fn fetch_songs_by_album_ids(
    _client: &Client,
    config: &StreamServerConfig,
    album_ids: Vec<String>,
) -> Result<Vec<SubsonicSong>, String> {
    if album_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Parallelize per-album requests to avoid N+1 sequential bottleneck.
    let results: Vec<Result<Vec<SubsonicSong>, String>> = album_ids
        .chunks(16)
        .flat_map(|chunk| {
            use rayon::prelude::*;
            let bclient = reqwest::blocking::Client::new();
            chunk
                .par_iter()
                .map(|album_id| {
                    let url = build_url(config, "getAlbum");
                    let mut params = generate_auth_params(config, true);
                    params.push(("id", album_id.to_string()));
                    let resp = bclient
                        .get(&url)
                        .query(&params)
                        .send()
                        .map_err(|e| format!("Request failed: {}", e))?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status()));
                    }
                    let root: Value = resp
                        .json()
                        .map_err(|e| format!("Parse error: {}", e))?;
                    let sub = root
                        .get("subsonic-response")
                        .and_then(|v| v.as_object())
                        .ok_or("Missing subsonic-response")?;
                    Ok(extract_album_songs(&Value::Object(sub.clone())))
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let mut songs = Vec::new();
    for result in results {
        match result {
            Ok(s) => songs.extend(s),
            Err(err) => {
                log::warn!("Subsonic album fetch failed: {}", err);
            }
        }
    }

    Ok(songs)
}

fn dedupe_songs_by_id(songs: Vec<SubsonicSong>) -> Vec<SubsonicSong> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for song in songs {
        if seen.insert(song.id.clone()) {
            deduped.push(song);
        }
    }
    deduped
}

fn is_auth_mechanism_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("provided authentication mechanism not supported")
        || lower.contains("authentication mechanism not supported")
}

async fn fetch_all_songs_once(config: &StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    let client = build_client()?;
    let mut search_error: Option<String> = None;
    log::debug!(
        "[subsonic-debug] scan_start server={} auth={} musicFolderId={}",
        config.server_url,
        if config.legacy_auth { "legacy" } else { "token" },
        config.music_folder_id.as_deref().unwrap_or("<none>")
    );

    match fetch_songs_via_search3(&client, config).await {
        Ok(songs) if !songs.is_empty() => {
            log::debug!("[subsonic-debug] strategy=search3 songs={}", songs.len());
            return Ok(dedupe_songs_by_id(songs)
                .into_iter()
                .map(|song| convert_song(&song, config))
                .collect());
        }
        Ok(_) => {
            log::debug!("[subsonic-debug] strategy=search3 songs=0 -> fallback");
        }
        Err(err) => {
            log::debug!("[subsonic-debug] strategy=search3 failed -> fallback, err={}", err);
            search_error = Some(err);
        }
    }

    match fetch_album_ids_via_album_list2(&client, config).await {
        Ok(album_ids) => {
            log::debug!("[subsonic-debug] strategy=getAlbumList2 album_ids={}", album_ids.len());
            match fetch_songs_by_album_ids(&client, config, album_ids).await {
                Ok(songs) if !songs.is_empty() => {
                    log::debug!("[subsonic-debug] strategy=getAlbumList2+getAlbum songs={}", songs.len());
                    return Ok(dedupe_songs_by_id(songs)
                        .into_iter()
                        .map(|song| convert_song(&song, config))
                        .collect());
                }
                Ok(_) => {
                    log::debug!("[subsonic-debug] strategy=getAlbumList2+getAlbum songs=0 -> artist fallback");
                }
                Err(err) => {
                    log::debug!("[subsonic-debug] strategy=getAlbumList2+getAlbum failed: {}", err);
                }
            }
        }
        Err(err) => {
            log::debug!("[subsonic-debug] strategy=getAlbumList2 failed: {}", err);
        }
    }

    let album_ids = fetch_album_ids_via_artists(&client, config).await?;
    log::debug!("[subsonic-debug] strategy=getArtists+getArtist album_ids={}", album_ids.len());
    let songs = fetch_songs_by_album_ids(&client, config, album_ids).await?;
    if !songs.is_empty() {
        log::debug!("[subsonic-debug] strategy=getArtists+getArtist+getAlbum songs={}", songs.len());
        return Ok(dedupe_songs_by_id(songs)
            .into_iter()
            .map(|song| convert_song(&song, config))
            .collect());
    }

    if let Some(err) = search_error {
        return Err(format!(
            "Failed to fetch songs (search3 failed and fallbacks returned no results): {}",
            err
        ));
    }
    Err("No songs found from search3 and all fallback paths".to_string())
}

pub async fn fetch_all_songs(config: &StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    match fetch_all_songs_once(config).await {
        Ok(songs) => Ok(songs),
        Err(err) if is_auth_mechanism_error(&err) => {
            let mut retry_config = config.clone();
            retry_config.legacy_auth = !config.legacy_auth;
            log::debug!(
                "Subsonic auth mode rejected, retrying with {} auth",
                if retry_config.legacy_auth {
                    "legacy"
                } else {
                    "token"
                }
            );
            fetch_all_songs_once(&retry_config)
                .await
                .map_err(|retry_err| format!("{}; retry failed: {}", err, retry_err))
        }
        Err(err) => Err(err),
    }
}

// ============ Playlists ============

/// Fetch playlist summaries from a Subsonic-compatible server (Navidrome/OpenSubsonic/Subsonic).
pub async fn fetch_playlists(config: &StreamServerConfig) -> Result<Vec<(String, String, u32, Option<String>)>, String> {
    let client = build_client()?;
    let url = build_url(config, "getPlaylists");
    let params = generate_auth_params(config, true);

    let response = client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("请求失败: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<SubsonicPlaylistsResponse> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "未知错误".to_string());
        return Err(msg);
    }

    let playlists = inner
        .data
        .and_then(|d| d.playlists)
        .and_then(|p| p.playlist)
        .map(|p| p.into_vec())
        .unwrap_or_default();

    Ok(playlists
        .into_iter()
        .map(|p| (p.id, p.name, p.song_count.unwrap_or(0), p.changed))
        .collect())
}

/// Fetch playlist track ids from a Subsonic-compatible server.
pub async fn fetch_playlist_song_ids(
    config: &StreamServerConfig,
    playlist_id: &str,
) -> Result<Vec<String>, String> {
    Ok(fetch_playlist_tracks(config, playlist_id)
        .await?
        .into_iter()
        .map(|(id, _, _, _, _)| id)
        .collect())
}

pub async fn fetch_playlist_tracks(
    config: &StreamServerConfig,
    playlist_id: &str,
) -> Result<Vec<(String, Option<String>, Option<String>, Option<String>, Option<String>)>, String> {
    let client = build_client()?;
    let url = build_url(config, "getPlaylist");
    let mut params = generate_auth_params(config, true);
    params.push(("id", playlist_id.to_string()));

    let response = client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("请求失败: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<SubsonicPlaylistResponse> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "未知错误".to_string());
        return Err(msg);
    }

    let entries = inner
        .data
        .and_then(|d| d.playlist)
        .and_then(|p| p.entry)
        .map(|e| e.into_vec())
        .unwrap_or_default();

    Ok(entries
        .into_iter()
        .map(|e| {
            let cover_url = e
                .cover_art
                .as_deref()
                .map(|cover_id| build_cover_art_url(config, cover_id));
            (e.id, e.title, e.artist, e.album, cover_url)
        })
        .collect())
}

/// Append songs to a Subsonic-compatible playlist.
pub async fn add_songs_to_playlist(
    config: &StreamServerConfig,
    playlist_id: &str,
    song_ids: &[String],
) -> Result<(), String> {
    if song_ids.is_empty() {
        return Ok(());
    }

    let client = build_client()?;
    let url = build_url(config, "updatePlaylist");
    let mut params = generate_auth_params(config, true);
    params.push(("playlistId", playlist_id.to_string()));
    for song_id in song_ids {
        params.push(("songIdToAdd", song_id.clone()));
    }

    let response = client
        .post(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Request failed: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(msg);
    }

    Ok(())
}

pub async fn create_playlist(
    config: &StreamServerConfig,
    name: &str,
    song_ids: &[String],
) -> Result<(), String> {
    let client = build_client()?;
    let url = build_url(config, "createPlaylist");
    let mut params = generate_auth_params(config, true);
    params.push(("name", name.to_string()));
    for song_id in song_ids {
        params.push(("songId", song_id.clone()));
    }

    let response = client
        .post(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Request failed: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(msg);
    }

    Ok(())
}

pub async fn rename_playlist(
    config: &StreamServerConfig,
    playlist_id: &str,
    name: &str,
) -> Result<(), String> {
    let client = build_client()?;
    let url = build_url(config, "updatePlaylist");
    let mut params = generate_auth_params(config, true);
    params.push(("playlistId", playlist_id.to_string()));
    params.push(("name", name.to_string()));

    let response = client
        .post(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Request failed: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(msg);
    }

    Ok(())
}

pub async fn delete_playlist(
    config: &StreamServerConfig,
    playlist_id: &str,
) -> Result<(), String> {
    let client = build_client()?;
    let url = build_url(config, "deletePlaylist");
    let mut params = generate_auth_params(config, true);
    params.push(("id", playlist_id.to_string()));

    let response = client
        .post(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Request failed: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(msg);
    }

    Ok(())
}

pub async fn remove_playlist_indexes(
    config: &StreamServerConfig,
    playlist_id: &str,
    indexes: &[usize],
) -> Result<(), String> {
    if indexes.is_empty() {
        return Ok(());
    }

    let client = build_client()?;
    let url = build_url(config, "updatePlaylist");
    let mut params = generate_auth_params(config, true);
    params.push(("playlistId", playlist_id.to_string()));
    for index in indexes {
        params.push(("songIndexToRemove", index.to_string()));
    }

    let response = client
        .post(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Request failed: HTTP {}", response.status()));
    }

    let data: SubsonicResponse<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        let msg = inner
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "Unknown error".to_string());
        return Err(msg);
    }

    Ok(())
}

/// Build stream URL for a song.
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let base = config.server_url.trim_end_matches('/');
    // Stream endpoint does not need `f=json`.
    let params = generate_auth_params(config, false);
    let query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}/rest/stream?id={}&{}", base, song_id, query)
}

/// OpenSubsonic structured lyrics payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLyricsBySongIdResponse {
    pub lyrics_list: Option<LyricsList>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsList {
    pub structured_lyrics: Option<Vec<StructuredLyrics>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredLyrics {
    pub synced: Option<bool>,
    pub line: Option<Vec<LyricLine>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub start: Option<u64>,
    pub value: Option<String>,
}

/// Get song lyrics.
pub async fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let client = build_client().ok()?;

    // First try `getLyricsBySongId` (OpenSubsonic extension, supports synced lyrics).
    let url = build_url(config, "getLyricsBySongId");
    let mut params = generate_auth_params(config, true);
    params.push(("id", song_id.to_string()));

    if let Ok(response) = client.get(&url).query(&params).send().await {
        if response.status().is_success() {
            if let Ok(data) = response
                .json::<SubsonicResponse<GetLyricsBySongIdResponse>>()
                .await
            {
                if data.subsonic_response.status == "ok" {
                    if let Some(lyrics_data) = data.subsonic_response.data {
                        if let Some(lyrics_list) = lyrics_data.lyrics_list {
                            if let Some(structured) = lyrics_list.structured_lyrics {
                                // Prefer synced lyrics.
                                for sl in &structured {
                                    if sl.synced == Some(true) {
                                        if let Some(lines) = &sl.line {
                                            let lrc = lines
                                                .iter()
                                                .filter_map(|l| {
                                                    let start = l.start.unwrap_or(0);
                                                    let value = l.value.as_ref()?;
                                                    let mins = start / 60000;
                                                    let secs = (start % 60000) / 1000;
                                                    let ms = (start % 1000) / 10;
                                                    Some(format!(
                                                        "[{:02}:{:02}.{:02}]{}",
                                                        mins, secs, ms, value
                                                    ))
                                                })
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            if !lrc.is_empty() {
                                                return Some(lrc);
                                            }
                                        }
                                    }
                                }
                                // Fallback to unsynced lyrics when no synced lyrics exist.
                                for sl in &structured {
                                    if let Some(lines) = &sl.line {
                                        let text = lines
                                            .iter()
                                            .filter_map(|l| l.value.as_ref())
                                            .cloned()
                                            .collect::<Vec<_>>()
                                            .join("\n");
                                        if !text.is_empty() {
                                            return Some(text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

