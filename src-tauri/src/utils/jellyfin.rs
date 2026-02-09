//! Jellyfin/Emby API 工具函数

use reqwest::Client;

use crate::models::{
    ConnectionTestResult, JellyfinAuthRequest, JellyfinAuthResponse, JellyfinItem,
    JellyfinItemsResponse, JellyfinLyricsResponse, JellyfinMediaStream, JellyfinSystemInfo,
    ScannedSong, ServerType, StreamServerConfig,
};
use crate::utils::audio::extract_filename_from_path_str;

/// 无损音频格式
const LOSSLESS_CONTAINERS: &[&str] = &["flac", "wav", "ape", "aiff", "dsf", "dff", "alac"];

/// 构建 Jellyfin/Emby 认证头
fn build_auth_header(config: &StreamServerConfig) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if let Some(token) = &config.access_token {
        headers.push(("Authorization".to_string(), format!(
            "MediaBrowser Client=\"BaYin\", Device=\"BaYin\", DeviceId=\"bayin-app\", Version=\"1.0.0\", Token=\"{}\"",
            token
        )));
    } else {
        headers.push(("Authorization".to_string(),
            "MediaBrowser Client=\"BaYin\", Device=\"BaYin\", DeviceId=\"bayin-app\", Version=\"1.0.0\"".to_string()
        ));
    }
    headers
}

fn base_url(config: &StreamServerConfig) -> String {
    config.server_url.trim_end_matches('/').to_string()
}

/// 认证并获取 access_token 和 user_id
pub async fn authenticate(config: &StreamServerConfig) -> Result<(String, String), String> {
    let client = Client::new();
    let url = format!("{}/Users/AuthenticateByName", base_url(config));

    let auth_headers = build_auth_header(config);
    let mut req = client.post(&url).json(&JellyfinAuthRequest {
        username: config.username.clone(),
        pw: config.password.clone(),
    });

    for (k, v) in &auth_headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let response = req.send().await.map_err(|e| format!("连接失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("认证失败: HTTP {}", response.status()));
    }

    let auth: JellyfinAuthResponse = response
        .json()
        .await
        .map_err(|e| format!("解析认证响应失败: {}", e))?;

    Ok((auth.access_token, auth.user.id))
}

/// 测试连接
pub async fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    // 先认证
    let (token, _user_id) = match authenticate(config).await {
        Ok(v) => v,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: e,
                server_version: None,
            }
        }
    };

    // 获取系统信息
    let client = Client::new();
    let url = format!("{}/System/Info/Public", base_url(config));

    match client.get(&url).send().await {
        Ok(resp) => {
            if let Ok(info) = resp.json::<JellyfinSystemInfo>().await {
                ConnectionTestResult {
                    success: true,
                    message: "连接成功".to_string(),
                    server_version: info.version,
                }
            } else {
                ConnectionTestResult {
                    success: true,
                    message: format!("连接成功 (token: {}...)", &token[..8.min(token.len())]),
                    server_version: None,
                }
            }
        }
        Err(_) => ConnectionTestResult {
            success: true,
            message: "认证成功".to_string(),
            server_version: None,
        },
    }
}

/// 将 Jellyfin 项转换为 ScannedSong
fn convert_item(item: &JellyfinItem, config: &StreamServerConfig) -> ScannedSong {
    let duration_secs = item
        .run_time_ticks
        .map(|t| t / 10_000_000)
        .unwrap_or(0);

    let container = item.container.as_deref().unwrap_or("");
    let is_sq = LOSSLESS_CONTAINERS.contains(&container.to_lowercase().as_str());

    // 从 media_streams 提取音频质量信息
    let audio_stream = item
        .media_sources
        .as_ref()
        .and_then(|sources| sources.first())
        .and_then(|s| s.media_streams.as_ref())
        .and_then(|streams| {
            streams
                .iter()
                .find(|s| s.stream_type.as_deref() == Some("Audio"))
        });

    let is_hr = audio_stream
        .map(|s: &JellyfinMediaStream| {
            s.sample_rate.map(|r| r > 44100).unwrap_or(false)
                || s.bit_depth.map(|d| d > 16).unwrap_or(false)
        })
        .unwrap_or(false);

    let artist = item
        .artists
        .as_ref()
        .and_then(|a| a.first().cloned())
        .or_else(|| item.album_artist.clone())
        .unwrap_or_else(|| "未知艺术家".to_string());

    // 构建封面 URL
    let cover_url = item.image_tags.as_ref().and_then(|tags| {
        if tags.contains_key("Primary") {
            let token = config.access_token.as_deref().unwrap_or("");
            Some(format!(
                "{}/Items/{}/Images/Primary?api_key={}",
                base_url(config),
                item.id,
                token
            ))
        } else {
            None
        }
    });

    let file_size = item
        .size
        .or_else(|| {
            item.media_sources
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|s| s.size)
        })
        .unwrap_or(0);

    // 标题：如果 name 为空，尝试从路径提取文件名
    let title = if item.name.is_empty() {
        item.path
            .as_ref()
            .and_then(|p| extract_filename_from_path_str(p))
            .unwrap_or_else(|| item.id.clone())
    } else {
        item.name.clone()
    };

    ScannedSong {
        id: item.id.clone(),
        title,
        artist,
        album: item
            .album
            .clone()
            .unwrap_or_else(|| "未知专辑".to_string()),
        duration: duration_secs as f64,
        file_path: item.path.clone().unwrap_or_default(),
        file_size,
        cover_url,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
    }
}

/// 获取所有音频项
pub async fn fetch_all_songs(config: &StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    let user_id = config
        .user_id
        .as_deref()
        .ok_or("缺少 userId，请先测试连接")?;
    let _token = config
        .access_token
        .as_deref()
        .ok_or("缺少 accessToken，请先测试连接")?;

    let client = Client::new();
    let url = format!("{}/Users/{}/Items", base_url(config), user_id);

    let mut all_songs = Vec::new();
    let mut start_index: u64 = 0;
    let page_size: u64 = 500;

    loop {
        let mut req = client
            .get(&url)
            .query(&[
                ("IncludeItemTypes", "Audio"),
                ("Recursive", "true"),
                ("Fields", "MediaSources,Path"),
                ("SortBy", "SortName"),
                ("SortOrder", "Ascending"),
            ])
            .query(&[("StartIndex", &start_index.to_string())])
            .query(&[("Limit", &page_size.to_string())]);

        let auth_headers = build_auth_header(config);
        for (k, v) in &auth_headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let response = req.send().await.map_err(|e| format!("请求失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("获取歌曲失败: HTTP {}", response.status()));
        }

        let data: JellyfinItemsResponse = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        let count = data.items.len() as u64;
        for item in &data.items {
            all_songs.push(convert_item(item, config));
        }

        start_index += count;
        if start_index >= data.total_record_count || count == 0 {
            break;
        }
    }

    Ok(all_songs)
}

/// 获取流 URL
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let token = config.access_token.as_deref().unwrap_or("");
    let base = base_url(config);

    if config.server_type == ServerType::Emby {
        format!(
            "{}/Audio/{}/universal?UserId={}&DeviceId=bayin-app&api_key={}&MaxStreamingBitrate=999999999&Container=opus,webm|opus,mp3,aac,m4a|aac,m4b|aac,flac,webma,webm|webma,wav,ogg&TranscodingContainer=mp4&TranscodingProtocol=hls&AudioCodec=aac&Static=true",
            base,
            song_id,
            config.user_id.as_deref().unwrap_or(""),
            token
        )
    } else {
        format!(
            "{}/Audio/{}/universal?UserId={}&DeviceId=bayin-app&api_key={}&MaxStreamingBitrate=999999999&Container=opus,webm|opus,mp3,aac,m4a|aac,m4b|aac,flac,webma,webm|webma,wav,ogg&TranscodingContainer=mp4&TranscodingProtocol=hls&AudioCodec=aac",
            base,
            song_id,
            config.user_id.as_deref().unwrap_or(""),
            token
        )
    }
}

/// 获取歌词
pub async fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let _token = config.access_token.as_deref()?;
    let client = Client::new();
    let url = format!("{}/Audio/{}/Lyrics", base_url(config), song_id);

    let auth_headers = build_auth_header(config);
    let mut req = client.get(&url);
    for (k, v) in &auth_headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let response = req.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let data: JellyfinLyricsResponse = response.json().await.ok()?;

    if data.lyrics.is_empty() {
        return None;
    }

    // 检查是否有时间戳（同步歌词）
    let has_timing = data.lyrics.iter().any(|l| l.start.is_some());

    if has_timing {
        let lrc = data
            .lyrics
            .iter()
            .filter_map(|l| {
                let ticks = l.start?;
                let ms = ticks / 10_000; // ticks to milliseconds
                let mins = ms / 60000;
                let secs = (ms % 60000) / 1000;
                let centis = (ms % 1000) / 10;
                Some(format!("[{:02}:{:02}.{:02}]{}", mins, secs, centis, l.text))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !lrc.is_empty() {
            return Some(lrc);
        }
    }

    // 纯文本歌词
    let text = data
        .lyrics
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if !text.is_empty() {
        Some(text)
    } else {
        None
    }
}
