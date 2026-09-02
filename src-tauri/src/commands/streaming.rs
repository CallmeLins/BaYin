use std::collections::HashMap;

use tauri::{Emitter, State};

use crate::commands::CoverCacheState;
use crate::db::{self, DbState};
use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::source;
use crate::utils::{jellyfin, subsonic, webdav};

// ============ 内部函数（供其他模块调用） ============

/// 从流媒体服务器获取所有歌曲（内部函数）
///
/// `existing_modified`: WebDAV 增量同步用（song_id → 已入库的修改时间）
/// `progress`: WebDAV 扫描进度通道（已处理数, 总数）
/// `read_tags`: true=逐首探测标签；false=快速扫描（文件名入库）
///
/// 已改用统一的 SourceConnector 分发层（`crate::source`），不再在命令层 if-else。
pub async fn fetch_stream_songs_internal(
    config: &StreamServerConfig,
    cover_cache: Option<std::sync::Arc<crate::utils::cover::CoverCache>>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<std::sync::mpsc::Sender<(usize, usize)>>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    // 连接器 trait 是同步的（内部 pollster 桥接既有 async utils），
    // 这里放入阻塞线程避免阻塞 async 运行时。
    let cfg = config.clone();
    let existing_owned = existing_modified.cloned();
    tauri::async_runtime::spawn_blocking(move || {
        source::fetch_songs(
            &cfg,
            cover_cache.as_deref(),
            existing_owned.as_ref(),
            progress,
            read_tags,
        )
    })
    .await
    .map_err(|e| e.to_string())?
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
    // 连接器 test_connection 是同步的（内部 pollster 桥接），放阻塞线程。
    let cfg = config.clone();
    tauri::async_runtime::spawn_blocking(move || source::test_connection(&cfg))
        .await
        .map_err(|e| e.to_string())
}

/// 从流媒体服务器获取所有歌曲
#[tauri::command]
pub async fn fetch_stream_songs(
    config: StreamServerConfig,
    cover_cache: State<'_, CoverCacheState>,
) -> Result<Vec<ScannedSong>, String> {
    let cache = cover_cache.0.lock().map_err(|e| e.to_string())?.clone_arc();
    fetch_stream_songs_internal(&config, Some(cache), None, None, false).await
}

/// 获取流媒体歌曲的流 URL
#[tauri::command]
pub fn get_stream_url(config: StreamServerConfig, song_id: String) -> String {
    source::get_stream_url(&config, &song_id)
}

/// 获取流媒体歌曲的流 URL（按已保存的服务器配置）
#[tauri::command]
pub fn get_stream_url_by_server(
    db: State<'_, DbState>,
    server_id: String,
    song_id: String,
) -> Result<String, String> {
    let config = load_stream_config(&db, &server_id)?;
    Ok(source::get_stream_url(&config, &song_id))
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
    let info = source::get_stream_play_info(&config, &song_id);
    Ok(StreamPlayInfo {
        url: info.url,
        headers: info.headers,
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

/// 下载 WebDAV 图片字节（文件夹封面等前端展示用，前端无法直接带认证头加载 <img>）
#[tauri::command]
pub async fn webdav_image(
    db: State<'_, DbState>,
    server_id: String,
    url: String,
) -> Result<Vec<u8>, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持图片加载".to_string());
    }
    let cfg = config.clone();
    let u = url.clone();
    tauri::async_runtime::spawn_blocking(move || webdav::fetch_bytes(&cfg, &u))
        .await
        .map_err(|e| e.to_string())?
}

/// 按歌曲 URL 返回其所在目录的封面（folder.jpg 等），无则 None。
/// 结果带进程内缓存 + 磁盘持久缓存；供歌曲列表无内嵌封面时回退显示。
#[tauri::command]
pub async fn webdav_folder_cover(
    db: State<'_, DbState>,
    cache_root: State<'_, crate::commands::CacheRootState>,
    server_id: String,
    song_url: String,
) -> Result<Option<String>, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Ok(None);
    }
    let cfg = config.clone();
    let sid = server_id.clone();
    let url = song_url.clone();
    let root = cache_root.0.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        webdav::folder_cover(&cfg, &sid, &url, Some(&root))
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(result)
}

/// 获取 WebDAV 服务器的根目录 URL（初始目录）
#[tauri::command]
pub async fn webdav_root_url(
    db: State<'_, DbState>,
    server_id: String,
) -> Result<String, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持目录浏览".to_string());
    }
    let base = webdav::clean_base_url(&config);
    let path = webdav::effective_initial_path_public(&config);
    Ok(format!("{}{}", base, path))
}

/// 删除 WebDAV 远程文件/文件夹，并同步清理库内记录与本地缓存
#[tauri::command]
pub async fn webdav_delete(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    cache_root: State<'_, crate::commands::CacheRootState>,
    server_id: String,
    url: String,
    is_directory: bool,
) -> Result<usize, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持删除".to_string());
    }

    // 1. 查询库内匹配的歌曲 URL（删除前先取，用于清理缓存）
    let song_urls = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_server_song_ids_by_url(&conn, &server_id, &url, is_directory)
            .map_err(|e| e.to_string())?
    };

    // 2. 删除远程资源
    let cfg = config.clone();
    let target = url.clone();
    tauri::async_runtime::spawn_blocking(move || webdav::delete(&cfg, &target))
        .await
        .map_err(|e| e.to_string())??;

    // 3. 清理本地缓存（按已知 URL 精确删除）
    let root = cache_root.0.clone();
    let sid = server_id.clone();
    let cache_removed = tauri::async_runtime::spawn_blocking(move || {
        let mut count = 0;
        for song_url in &song_urls {
            let path = webdav::cached_path(&root, &sid, song_url);
            if path.is_file() && std::fs::remove_file(&path).is_ok() {
                count += 1;
            }
        }
        count
    })
    .await
    .map_err(|e| e.to_string())?;

    // 4. 清理库内记录
    let removed = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if is_directory {
            db::songs::delete_songs_by_server_url_prefix(&conn, &server_id, &url)
                .map_err(|e| e.to_string())?
        } else {
            db::songs::delete_songs_by_server_song_id(&conn, &server_id, &url)
                .map_err(|e| e.to_string())?
        }
    };
    let _ = app.emit("library-updated", ());

    Ok(removed + cache_removed)
}

/// 移动/重命名 WebDAV 远程文件或文件夹，同步更新库记录与本地缓存文件名
#[tauri::command]
pub async fn webdav_move(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    cache_root: State<'_, crate::commands::CacheRootState>,
    server_id: String,
    source: String,
    destination: String,
    is_directory: bool,
) -> Result<usize, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持移动".to_string());
    }

    // 1. 先取旧 URL 列表（DB 更新后旧 URL 将无法查询）
    let old_urls = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_server_song_ids_by_url(&conn, &server_id, &source, is_directory)
            .map_err(|e| e.to_string())?
    };

    // 2. 移动远程资源
    let cfg = config.clone();
    let src = source.clone();
    let dst = destination.clone();
    tauri::async_runtime::spawn_blocking(move || webdav::move_entry(&cfg, &src, &dst))
        .await
        .map_err(|e| e.to_string())??;

    // 3. 重命名本地缓存文件（按新旧 URL 对应）
    let root = cache_root.0.clone();
    let sid = server_id.clone();
    let src2 = source.clone();
    let dst2 = destination.clone();
    let cache_renamed = tauri::async_runtime::spawn_blocking(move || {
        let mut count = 0;
        for old_url in &old_urls {
            let new_url = if is_directory {
                old_url.replacen(&src2, &dst2, 1)
            } else {
                dst2.clone()
            };
            let old_path = webdav::cached_path(&root, &sid, old_url);
            let new_path = webdav::cached_path(&root, &sid, &new_url);
            if old_path.is_file() && std::fs::rename(&old_path, &new_path).is_ok() {
                count += 1;
            }
        }
        count
    })
    .await
    .map_err(|e| e.to_string())?;

    // 4. 更新库内记录
    let updated = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if is_directory {
            db::songs::rename_server_song_id_prefix(&conn, &server_id, &source, &destination)
                .map_err(|e| e.to_string())?
        } else {
            db::songs::rename_server_song_id(&conn, &server_id, &source, &destination)
                .map_err(|e| e.to_string())?
        }
    };
    let _ = app.emit("library-updated", ());

    Ok(updated + cache_renamed)
}

/// 上传本地文件到 WebDAV 当前目录，返回远程 URL
#[tauri::command]
pub async fn webdav_upload(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    server_id: String,
    dir_url: String,
    local_path: String,
) -> Result<String, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持上传".to_string());
    }

    // 目标 URL = 目录 URL + 文件名（自动百分号编码）
    let file_name = std::path::Path::new(&local_path)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无法获取文件名".to_string())?;
    let target = {
        let mut u = url::Url::parse(dir_url.trim_end_matches('/'))
            .map_err(|e| format!("URL 无效: {e}"))?;
        u.path_segments_mut()
            .map_err(|_| "URL 无路径".to_string())?
            .push(file_name);
        u.to_string()
    };

    let cfg = config.clone();
    let remote = target.clone();
    let local = local_path.clone();
    tauri::async_runtime::spawn_blocking(move || webdav::upload(&cfg, &local, &remote))
        .await
        .map_err(|e| e.to_string())??;

    let _ = app.emit("library-updated", ());
    Ok(target)
}

/// 后台补全 WebDAV 歌曲标签（快速扫描入库后自动调用），返回更新的歌曲数
#[tauri::command]
pub async fn webdav_backfill_metadata(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    cover_cache: State<'_, CoverCacheState>,
    server_id: String,
) -> Result<usize, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Ok(0);
    }

    // 查需要补全的歌曲（duration = 0）
    let pending = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_stream_songs_needing_tags(&conn, &server_id).map_err(|e| e.to_string())?
    };
    if pending.is_empty() {
        return Ok(0);
    }

    // 探测放阻塞线程（复用连接池 + 8 并发），DB 更新回到 async 上下文
    let cache_arc = cover_cache.0.lock().map_err(|e| e.to_string())?.clone_arc();
    let cfg = config.clone();
    let results = tauri::async_runtime::spawn_blocking(move || {
        use rayon::prelude::*;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .build()
            .map_err(|e| e.to_string())?;
        let results: Vec<(String, Option<crate::models::ScannedSong>)> = pool.install(|| {
            pending
                .par_iter()
                .map(|(id, url)| (id.clone(), webdav::probe_song(&cfg, Some(&cache_arc), url)))
                .collect()
        });
        Ok::<_, String>(results)
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut updated = 0;
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        for (id, song) in &results {
            if let Some(song) = song {
                if db::songs::update_song_tags(&conn, id, song).map_err(|e| e.to_string())? > 0 {
                    updated += 1;
                }
            }
        }
    }
    if updated > 0 {
        let _ = app.emit("library-updated", ());
    }
    Ok(updated)
}

/// 将 WebDAV 指定目录扫描入库（文件夹浏览页“加入音乐库”），返回入库歌曲数。
/// 只增不删，已存在的歌曲（同 URL）会更新并保留播放统计。
#[tauri::command]
pub async fn webdav_scan_dir_to_db(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    cover_cache: State<'_, CoverCacheState>,
    server_id: String,
    dir_url: String,
) -> Result<usize, String> {
    let config = load_stream_config(&db, &server_id)?;
    if !config.is_webdav() {
        return Err("仅 WebDAV 服务器支持目录浏览".to_string());
    }

    // 增量同步：复用库内已有元数据，跳过未变化文件的探测
    let existing_metadata = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_stream_metadata_by_source(&conn, "stream", Some(&server_id))
            .map_err(|e| e.to_string())?
    };
    let existing_modified: std::collections::HashMap<String, i64> = existing_metadata
        .iter()
        .filter_map(|(id, meta)| meta.file_modified.map(|m| (id.clone(), m)))
        .collect();

    let cache_arc = cover_cache.0.lock().map_err(|e| e.to_string())?.clone_arc();
    let cfg = config.clone();
    let url = dir_url.clone();
    let songs = tauri::async_runtime::spawn_blocking(move || {
        webdav::scan_dir(&cfg, Some(&cache_arc), Some(&existing_modified), &url, None)
    })
    .await
    .map_err(|e| e.to_string())??;

    // 保存歌曲（保留播放统计 / created_at，不删除现有歌曲）
    let existing_user_stats = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_user_stats_by_source(&conn, "stream", Some(&server_id))
            .map_err(|e| e.to_string())?
    };
    let existing_cover_hashes = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_cover_hashes_by_source(&conn, "stream", Some(&server_id))
            .map_err(|e| e.to_string())?
    };
    let existing_created_ats = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::get_created_ats_by_source(&conn, "stream", Some(&server_id))
            .map_err(|e| e.to_string())?
    };

    let song_inputs: Vec<db::SongInput> = songs
        .iter()
        .map(|s| {
            let id = format!("{}-{}", server_id, s.id);
            let existing = existing_metadata.get(&id);
            let use_existing = existing
                .map(|meta| meta.file_modified == s.file_modified)
                .unwrap_or(false);
            let (title, artist, album, duration) = if use_existing {
                let meta = existing.expect("checked above");
                (
                    meta.title.clone(),
                    meta.artist.clone(),
                    meta.album.clone(),
                    meta.duration,
                )
            } else {
                (s.title.clone(), s.artist.clone(), s.album.clone(), s.duration)
            };
            db::SongInput {
                id: id.clone(),
                title,
                artist,
                album,
                duration,
                file_path: String::new(),
                file_size: s.file_size as i64,
                is_hr: s.is_hr,
                is_sq: s.is_sq,
                cover_hash: existing_cover_hashes
                    .get(&id)
                    .cloned()
                    .or_else(|| s.cover_hash.clone()),
                server_song_id: Some(s.id.clone()),
                stream_info: Some(
                    serde_json::json!({
                        "type": "stream",
                        "serverId": server_id,
                        "serverType": "webdav",
                        "songId": s.id,
                        "serverName": config.server_name,
                        "coverUrl": s.cover_url
                    })
                    .to_string(),
                ),
                file_modified: s.file_modified,
                format: s.format.clone(),
                bit_depth: s.bit_depth,
                sample_rate: s.sample_rate,
                bitrate: s.bitrate,
                channels: s.channels,
                lyrics: None,
                created_at: s.created_at.or(existing_created_ats.get(&id).copied()),
            }
        })
        .collect();

    {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        let saved = db::songs::save_songs(&mut conn, &song_inputs, "stream", Some(&server_id))
            .map_err(|e| e.to_string())?;
        if !existing_user_stats.is_empty() {
            db::songs::restore_user_stats(&mut conn, &existing_user_stats)
                .map_err(|e| e.to_string())?;
        }
        let _ = app.emit("library-updated", ());
        Ok(saved)
    }
}

/// 回写流媒体歌曲的播放时长（快速扫描的歌播放后补齐，仅 WebDAV/流媒体）
#[tauri::command]
pub fn update_stream_song_duration(
    db: State<'_, DbState>,
    server_id: String,
    song_id: String,
    duration: f64,
) -> Result<usize, String> {
    if duration <= 0.0 {
        return Ok(0);
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::songs::update_song_duration_by_server(&conn, &server_id, &song_id, duration)
        .map_err(|e| e.to_string())
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

/// 批量查询已缓存的 WebDAV 歌曲 URL（文件夹浏览页徽标用）
#[tauri::command]
pub fn get_cached_stream_paths(
    cache_root: State<'_, crate::commands::CacheRootState>,
    server_id: String,
    song_urls: Vec<String>,
) -> Vec<String> {
    song_urls
        .into_iter()
        .filter(|u| webdav::has_cached_song(&cache_root.0, &server_id, u))
        .collect()
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
    // 连接器 get_lyrics 是同步的（内部 pollster 桥接），放阻塞线程避免阻塞 async 运行时。
    let cfg = config.clone();
    tauri::async_runtime::spawn_blocking(move || source::get_lyrics(&cfg, &song_id))
        .await
        .ok()
        .flatten()
}

/// 获取流媒体歌曲歌词（按已保存的服务器配置）
/// 歌词获取一次后存入数据库缓存，重新扫描/重启不丢失，仅清库时清除。
#[tauri::command]
pub async fn get_stream_lyrics_by_server(
    db: State<'_, DbState>,
    server_id: String,
    song_id: String,
) -> Result<Option<String>, String> {
    let config = load_stream_config(&db, &server_id)?;

    // 1. DB 缓存命中直接返回
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = db::songs::get_lyrics_by_song(&conn, &server_id, &song_id)
            .map_err(|e| e.to_string())?
        {
            return Ok(Some(cached));
        }
    }

    // 2. 未缓存则从服务器获取（连接器 get_lyrics 同步，放阻塞线程）
    let cfg = config.clone();
    let sid = song_id.clone();
    let lyrics = tauri::async_runtime::spawn_blocking(move || source::get_lyrics(&cfg, &sid))
        .await
        .ok()
        .flatten();

    // 3. 获取成功则写入缓存
    if let Some(l) = &lyrics {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::songs::save_lyrics(&conn, &server_id, &song_id, l).map_err(|e| e.to_string())?;
    }
    Ok(lyrics)
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
