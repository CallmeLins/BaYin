//! Subsonic API 工具函数
//! 支持 Navidrome、Subsonic、OpenSubsonic 等兼容服务器
#![allow(dead_code)]

use rand::Rng;
use reqwest::Client;
use serde::Deserialize;

use crate::models::{
    ConnectionTestResult, GetAlbumListResponse, GetAlbumResponse, StreamServerConfig, PingResponse,
    ScannedSong, SearchResponse, SubsonicResponse, SubsonicSong,
};
use crate::utils::audio::extract_filename_from_path_str;

/// 无损音频格式
const LOSSLESS_SUFFIXES: &[&str] = &["flac", "wav", "ape", "aiff", "dsf", "dff", "alac"];

/// 生成 Subsonic API 认证参数
fn generate_auth_params(config: &StreamServerConfig) -> Vec<(&str, String)> {
    let salt: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    let token = format!("{:x}", md5::compute(format!("{}{}", config.password, salt)));

    vec![
        ("u", config.username.clone()),
        ("t", token),
        ("s", salt),
        ("v", "1.16.1".to_string()),
        ("c", "BaYin".to_string()),
        ("f", "json".to_string()),
    ]
}

/// 构建 API URL
fn build_url(config: &StreamServerConfig, endpoint: &str) -> String {
    let base = config.server_url.trim_end_matches('/');
    format!("{}/rest/{}", base, endpoint)
}

/// 测试服务器连接
pub async fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    let client = Client::new();
    let url = build_url(config, "ping");
    let params = generate_auth_params(config);

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

/// 将 Subsonic 歌曲转换为 ScannedSong
fn convert_song(song: &SubsonicSong, config: &StreamServerConfig) -> ScannedSong {
    let suffix = song.suffix.as_deref().unwrap_or("");
    let is_sq = LOSSLESS_SUFFIXES.contains(&suffix.to_lowercase().as_str());
    let is_hr = song.sampling_rate.map(|r| r > 44100).unwrap_or(false)
        || song.bit_depth.map(|d| d > 16).unwrap_or(false);

    // 构建封面 URL
    let cover_url = song.cover_art.as_ref().map(|cover_id| {
        let base = config.server_url.trim_end_matches('/');
        let params = generate_auth_params(config);
        let query: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}/rest/getCoverArt?id={}&{}", base, cover_id, query)
    });

    // 标题：如果 title 为空，尝试从路径提取文件名
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
            .unwrap_or_else(|| "未知艺术家".to_string()),
        album: song.album.clone().unwrap_or_else(|| "未知专辑".to_string()),
        duration: song.duration.unwrap_or(0) as f64,
        file_path: song.path.clone().unwrap_or_default(),
        file_size: song.size.unwrap_or(0),
        cover_url,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
    }
}

/// 获取所有歌曲（通过搜索所有）
pub async fn fetch_all_songs(config: &StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    let client = Client::new();
    let mut all_songs = Vec::new();

    // 使用 search3 获取所有歌曲
    let url = build_url(config, "search3");
    let mut params = generate_auth_params(config);
    params.push(("query", "".to_string())); // 空查询获取所有
    params.push(("songCount", "10000".to_string()));
    params.push(("albumCount", "0".to_string()));
    params.push(("artistCount", "0".to_string()));

    let response = client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let data: SubsonicResponse<SearchResponse> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        if let Some(error) = inner.error {
            return Err(format!("API 错误: {}", error.message));
        }
        return Err("未知错误".to_string());
    }

    if let Some(search_result) = inner.data {
        if let Some(result) = search_result.search_result3 {
            if let Some(songs) = result.song {
                for song in &songs {
                    all_songs.push(convert_song(song, config));
                }
            }
        }
    }

    Ok(all_songs)
}

/// 获取专辑列表
pub async fn fetch_albums(
    config: &StreamServerConfig,
) -> Result<Vec<crate::models::SubsonicAlbum>, String> {
    let client = Client::new();
    let url = build_url(config, "getAlbumList2");
    let mut params = generate_auth_params(config);
    params.push(("type", "alphabeticalByName".to_string()));
    params.push(("size", "500".to_string()));

    let response = client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let data: SubsonicResponse<GetAlbumListResponse> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        if let Some(error) = inner.error {
            return Err(format!("API 错误: {}", error.message));
        }
        return Err("未知错误".to_string());
    }

    if let Some(album_list_data) = inner.data {
        if let Some(album_list) = album_list_data.album_list2 {
            return Ok(album_list.album.unwrap_or_default());
        }
    }

    Ok(Vec::new())
}

/// 获取专辑中的所有歌曲
pub async fn fetch_album_songs(
    config: &StreamServerConfig,
    album_id: &str,
) -> Result<Vec<ScannedSong>, String> {
    let client = Client::new();
    let url = build_url(config, "getAlbum");
    let mut params = generate_auth_params(config);
    params.push(("id", album_id.to_string()));

    let response = client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let data: SubsonicResponse<GetAlbumResponse> = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let inner = data.subsonic_response;
    if inner.status != "ok" {
        if let Some(error) = inner.error {
            return Err(format!("API 错误: {}", error.message));
        }
        return Err("未知错误".to_string());
    }

    if let Some(album_data) = inner.data {
        if let Some(album) = album_data.album {
            if let Some(songs) = album.song {
                return Ok(songs.iter().map(|s| convert_song(s, config)).collect());
            }
        }
    }

    Ok(Vec::new())
}

/// 获取歌曲流 URL
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let base = config.server_url.trim_end_matches('/');
    // 流媒体请求不需要 f=json 参数
    let salt: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    let token = format!("{:x}", md5::compute(format!("{}{}", config.password, salt)));
    let params = vec![
        ("u", config.username.clone()),
        ("t", token),
        ("s", salt),
        ("v", "1.16.1".to_string()),
        ("c", "BaYin".to_string()),
    ];
    let query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}/rest/stream?id={}&{}", base, song_id, query)
}

/// 获取歌词响应
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLyricsResponse {
    pub lyrics: Option<LyricsData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsData {
    pub value: Option<String>,
}

/// 获取结构化歌词响应 (OpenSubsonic 扩展)
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

/// 获取歌曲歌词
pub async fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let client = Client::new();

    // 首先尝试 getLyricsBySongId (OpenSubsonic 扩展，支持同步歌词)
    let url = build_url(config, "getLyricsBySongId");
    let mut params = generate_auth_params(config);
    params.push(("id", song_id.to_string()));

    if let Ok(response) = client.get(&url).query(&params).send().await {
        if response.status().is_success() {
            if let Ok(data) = response.json::<SubsonicResponse<GetLyricsBySongIdResponse>>().await {
                if data.subsonic_response.status == "ok" {
                    if let Some(lyrics_data) = data.subsonic_response.data {
                        if let Some(lyrics_list) = lyrics_data.lyrics_list {
                            if let Some(structured) = lyrics_list.structured_lyrics {
                                // 优先使用同步歌词
                                for sl in &structured {
                                    if sl.synced == Some(true) {
                                        if let Some(lines) = &sl.line {
                                            let lrc = lines.iter()
                                                .filter_map(|l| {
                                                    let start = l.start.unwrap_or(0);
                                                    let value = l.value.as_ref()?;
                                                    let mins = start / 60000;
                                                    let secs = (start % 60000) / 1000;
                                                    let ms = (start % 1000) / 10;
                                                    Some(format!("[{:02}:{:02}.{:02}]{}", mins, secs, ms, value))
                                                })
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            if !lrc.is_empty() {
                                                return Some(lrc);
                                            }
                                        }
                                    }
                                }
                                // 如果没有同步歌词，使用非同步歌词
                                for sl in &structured {
                                    if let Some(lines) = &sl.line {
                                        let text = lines.iter()
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
