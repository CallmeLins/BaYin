use std::collections::HashMap;

use tauri::State;

use crate::commands::CoverCacheState;
use crate::db::{self, DbState};
use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::{jellyfin, subsonic, webdav};

// ============ 内部函数（供其他模块调用） ============

/// 从流媒体服务器获取所有歌曲（内部函数）
///
/// `existing_modified`: WebDAV 增量同步用（song_id → 已入库的修改时间）
pub async fn fetch_stream_songs_internal(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
) -> Result<Vec<ScannedSong>, String> {
    if config.is_subsonic() {
        subsonic::fetch_all_songs(config).await
    } else if config.is_webdav() {
        webdav::fetch_all_songs(config, cover_cache, existing_modified).await
    } else {
        jellyfin::fetch_all_songs(config).await
    }
}

fn load_stream_config(
    db: &State<'_, DbState>,
    server_id: &str,
) -> Result<StreamServerConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let server = db::servers::get_stream_server_by_id(&conn, server_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("流媒体服务器不存在: {server_id}"))?;
    Ok(StreamServerConfig::from(&server))
}

// ============ 统一命令（新） ============

/// 测试流媒体服务器连接
#[tauri::command]
pub async fn test_stream_connection(config: StreamServerConfig) -> Result<ConnectionTestResult, String> {
    if config.is_subsonic() {
        Ok(subsonic::test_connection(&config).await)
    } else if config.is_webdav() {
        Ok(webdav::test_connection(&config).await)
    } else {
        Ok(jellyfin::test_connection(&config).await)
    }
}

/// 从流媒体服务器获取所有歌曲
#[tauri::command]
pub async fn fetch_stream_songs(
    config: StreamServerConfig,
    cover_cache: State<'_, CoverCacheState>,
) -> Result<Vec<ScannedSong>, String> {
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?.clone_arc();
    fetch_stream_songs_internal(&config, Some(&cache), None).await
}

/// 获取流媒体歌曲的流 URL
#[tauri::command]
pub fn get_stream_url(config: StreamServerConfig, song_id: String) -> String {
    if config.is_subsonic() {
        subsonic::get_stream_url(&config, &song_id)
    } else if config.is_webdav() {
        webdav::stream_url(&config, &song_id)
    } else {
        jellyfin::get_stream_url(&config, &song_id)
    }
}

/// 获取流媒体歌曲的流 URL（按已保存的服务器配置）
#[tauri::command]
pub fn get_stream_url_by_server(
    db: State<'_, DbState>,
    server_id: String,
    song_id: String,
) -> Result<String, String> {
    let config = load_stream_config(&db, &server_id)?;
    Ok(if config.is_subsonic() {
        subsonic::get_stream_url(&config, &song_id)
    } else if config.is_webdav() {
        webdav::stream_url(&config, &song_id)
    } else {
        jellyfin::get_stream_url(&config, &song_id)
    })
}

/// 流媒体播放信息：URL + 附加请求头（如 WebDAV Basic Auth）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPlayInfo {
    pub url: String,
    /// 附加到 HTTP 音源的请求头，无认证时为空
    pub headers: Option<Vec<(String, String)>>,
}

/// 获取流媒体歌曲的播放信息（URL + 认证请求头，按已保存的服务器配置）
#[tauri::command]
pub fn get_stream_play_info_by_server(
    db: State<'_, DbState>,
    server_id: String,
    song_id: String,
) -> Result<StreamPlayInfo, String> {
    let config = load_stream_config(&db, &server_id)?;
    let url = if config.is_subsonic() {
        subsonic::get_stream_url(&config, &song_id)
    } else if config.is_webdav() {
        webdav::stream_url(&config, &song_id)
    } else {
        jellyfin::get_stream_url(&config, &song_id)
    };
    Ok(StreamPlayInfo {
        url,
        headers: if config.is_webdav() {
            Some(webdav::stream_headers(&config))
        } else {
            None
        },
    })
}

// ============ WebDAV 本地缓存 ============

/// 列出 WebDAV 服务器指定目录的内容（文件夹浏览用）
#[tauri::command]
pub async fn webdav_list_dir(
    db: State<'_, DbState>,
    server_id: String,
    dir_url: Option<String>,
) -> Result<Vec<webdav::WebDavEntry>, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持目录浏览".to_string());
    }
    let url = dir_url.clone();
    let cfg = config.clone();
    tauri::async_runtime::spawn_blocking(move || webdav::list_dir_entries(&cfg, url.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// 统计 WebDAV 歌曲本地缓存占用
#[tauri::command]
pub fn get_webdav_cache_stats(
    cache_root: State<'_, crate::commands::CacheRootState>,
) -> webdav::WebDavCacheStats {
    webdav::cache_stats(&cache_root.0)
}

/// 清理 WebDAV 歌曲本地缓存，返回删除的文件数
#[tauri::command]
pub fn clear_webdav_cache(
    cache_root: State<'_, crate::commands::CacheRootState>,
) -> usize {
    webdav::clear_cache(&cache_root.0)
}

/// 获取 WebDAV 歌曲的本地缓存路径（未缓存时返回 None）
#[tauri::command]
pub fn get_cached_stream_path(
    cache_root: State<'_, crate::commands::CacheRootState>,
    server_id: String,
    song_id: String,
) -> Option<String> {
    let cached = webdav::cached_path(&cache_root.0, &server_id, &song_id);
    cached.exists().then(|| cached.to_string_lossy().to_string())
}

/// 下载 WebDAV 歌曲到本地缓存（播放过半时触发），返回缓存路径
#[tauri::command]
pub async fn cache_stream_song(
    db: State<'_, DbState>,
    cache_root: State<'_, crate::commands::CacheRootState>,
    server_id: String,
    song_id: String,
) -> Result<Option<String>, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Ok(None);
    }
    // 下载放阻塞线程，避免阻塞 async 运行时
    let root = cache_root.0.clone();
    let cfg = config.clone();
    let sid = server_id.clone();
    let song = song_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        webdav::cache_song(&cfg, &root, &sid, &song)
    })
    .await
    .map_err(|e| e.to_string())?;
    result.map(Some).map_err(|e| e.to_string())
}

/// 获取流媒体歌曲歌词
#[tauri::command]
pub async fn get_stream_lyrics(config: StreamServerConfig, song_id: String) -> Option<String> {
    if config.is_subsonic() {
        subsonic::get_lyrics(&config, &song_id).await
    } else if config.is_webdav() {
        // 尝试读取同目录的 .lrc 侧车文件
        let cfg = config.clone();
        let sid = song_id.clone();
        tauri::async_runtime::spawn_blocking(move || webdav::get_lyrics(&cfg, &sid))
            .await
            .ok()
            .flatten()
    } else {
        jellyfin::get_lyrics(&config, &song_id).await
    }
}

/// 获取流媒体歌曲歌词（按已保存的服务器配置）
#[tauri::command]
pub async fn get_stream_lyrics_by_server(
    db: State<'_, DbState>,
    server_id: String,
    song_id: String,
) -> Result<Option<String>, String> {
    let config = load_stream_config(&db, &server_id)?;
    Ok(if config.is_subsonic() {
        subsonic::get_lyrics(&config, &song_id).await
    } else if config.is_webdav() {
        let cfg = config.clone();
        let sid = song_id.clone();
        tauri::async_runtime::spawn_blocking(move || webdav::get_lyrics(&cfg, &sid))
            .await
            .ok()
            .flatten()
    } else {
        jellyfin::get_lyrics(&config, &song_id).await
    })
}

/// Jellyfin/Emby 认证并返回 token 和 userId
#[tauri::command]
pub async fn jellyfin_authenticate(config: StreamServerConfig) -> Result<(String, String), String> {
    if config.is_jellyfin_like() {
        jellyfin::authenticate(&config).await
    } else {
        Err("此命令仅适用于 Jellyfin/Emby 服务器".to_string())
    }
}

// ============ 向后兼容的旧命令（Subsonic API） ============

/// 测试 Subsonic 服务器连接
#[tauri::command]
pub async fn test_subsonic_connection(config: StreamServerConfig) -> Result<ConnectionTestResult, String> {
    Ok(subsonic::test_connection(&config).await)
}

/// 从 Subsonic 服务器获取所有歌曲
#[tauri::command]
pub async fn fetch_subsonic_songs(config: StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    subsonic::fetch_all_songs(&config).await
}

/// 获取 Subsonic 歌曲流 URL
#[tauri::command]
pub fn get_subsonic_stream_url(config: StreamServerConfig, song_id: String) -> String {
    subsonic::get_stream_url(&config, &song_id)
}

/// 获取 Subsonic 歌曲歌词
#[tauri::command]
pub async fn get_subsonic_lyrics(config: StreamServerConfig, song_id: String) -> Option<String> {
    subsonic::get_lyrics(&config, &song_id).await
}
