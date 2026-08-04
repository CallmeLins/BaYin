//! WebDAV 音乐库支持
//!
//! 通过 PROPFIND 递归扫描远程目录，读取音频标签元数据，
//! 歌曲 ID 使用完整可播放 URL，播放时附带 Basic Auth 请求头。
//! 纯 Rust + reqwest + roxmltree，跨平台可用。

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use lofty::file::AudioFile;
use lofty::prelude::*;
use lofty::probe::Probe;
use rayon::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender as ProgressSender;
use std::time::Duration;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::cover::CoverCache;
use sha2::{Digest, Sha256};

/// 支持的音频扩展名（与扫描器保持一致）
const AUDIO_EXTS: &[&str] = &[
    "flac", "mp3", "m4a", "m4b", "aac", "ogg", "oga", "opus", "wav", "aiff", "aif", "ape",
    "wv", "mp4", "mka", "tak", "tta", "mp2",
];

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 元数据探测下载的头部大小（覆盖 FLAC 头 / MP3 ID3v2 / MP4 moov 常见大小）
const METADATA_PROBE_BYTES: u64 = 1024 * 1024;

/// WebDAV 目录项
#[derive(Debug, Clone)]
pub struct WebDavFile {
    /// 完整可访问路径（cleanBaseUrl + href）
    pub url: String,
    /// 文件名或目录名
    pub name: String,
    pub is_directory: bool,
    /// RFC 1123 修改时间（可能缺失）
    pub modified: Option<i64>,
    /// 文件大小（可能缺失）
    pub size: Option<u64>,
}

/// 拆分后的服务器地址：协议+主机 与 初始路径
#[derive(Debug, Clone)]
pub struct WebDavTarget {
    pub clean_base_url: String,
    pub initial_path: String,
}

/// 解析服务器地址为 WebDAV 目标。
/// 例: `https://host:port/dav/music` → base=`https://host:port` path=`/dav/music`
fn parse_target(config: &StreamServerConfig) -> WebDavTarget {
    let raw = config.server_url.trim_end_matches('/');
    let parsed = match url::Url::parse(raw) {
        Ok(u) => u,
        Err(_) => {
            // 用户可能没写协议，补 https 再解析
            return match url::Url::parse(&format!("https://{raw}")) {
                Ok(u) => WebDavTarget {
                    clean_base_url: format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), port_suffix(&u)),
                    initial_path: normalize_initial_path(u.path()),
                },
                Err(_) => WebDavTarget {
                    clean_base_url: raw.to_string(),
                    initial_path: String::new(),
                },
            };
        }
    };

    WebDavTarget {
        clean_base_url: format!("{}://{}{}", parsed.scheme(), parsed.host_str().unwrap_or(""), port_suffix(&parsed)),
        initial_path: normalize_initial_path(parsed.path()),
    }
}

/// 端口后缀（有非默认端口时返回 `:port`）
fn port_suffix(u: &url::Url) -> String {
    u.port()
        .map(|p| format!(":{p}"))
        .unwrap_or_default()
}

/// 初始目录 = server_url 的路径部分；若配置了 base_path 则以 base_path 为准
fn effective_initial_path(config: &StreamServerConfig) -> String {
    let fallback = parse_target(config).initial_path;
    match &config.base_path {
        Some(p) if !p.trim().is_empty() => normalize_initial_path(p),
        _ => fallback,
    }
}

/// 根目录 URL（供前端移动/上传目标计算）
pub fn effective_initial_path_public(config: &StreamServerConfig) -> String {
    effective_initial_path(config)
}

/// 基础 URL（协议+主机+端口）
pub fn clean_base_url(config: &StreamServerConfig) -> String {
    parse_target(config).clean_base_url
}

fn normalize_initial_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 构建带 Basic Auth 的请求头
fn build_auth_headers(config: &StreamServerConfig) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let token = BASE64.encode(format!("{}:{}", config.username, config.password));
    if let Ok(v) = HeaderValue::from_str(&format!("Basic {token}")) {
        headers.insert(AUTHORIZATION, v);
    }
    headers
}

fn build_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))
}

/// PROPFIND 深度 0，测试连接是否可用。
/// 注意：blocking 客户端必须在阻塞线程执行，否则 tokio 会 panic。
pub async fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    let cfg = config.clone();
    tauri::async_runtime::spawn_blocking(move || test_connection_blocking(&cfg))
        .await
        .unwrap_or_else(|_| ConnectionTestResult {
            success: false,
            message: "连接测试任务失败".to_string(),
            server_version: None,
        })
}

fn test_connection_blocking(config: &StreamServerConfig) -> ConnectionTestResult {
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: e,
                server_version: None,
            }
        }
    };
    let headers = build_auth_headers(config);
    let path = effective_initial_path(config);
    let url = format!("{}{}", parse_target(config).clean_base_url, path);

    let resp = match client
        .request(propfind_method(), &url)
        .headers(headers)
        .header("Depth", "0")
        .header(ACCEPT, "application/xml")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: format!("连接失败: {e}"),
                server_version: None,
            }
        }
    };

    let status = resp.status().as_u16();
    if (200..400).contains(&status) {
        ConnectionTestResult {
            success: true,
            message: "连接成功".to_string(),
            server_version: None,
        }
    } else {
        ConnectionTestResult {
            success: false,
            message: format!("服务器返回状态码 {status}"),
            server_version: None,
        }
    }
}

/// reqwest 无 PROPFIND 常量，用 from_bytes 构造
fn propfind_method() -> reqwest::Method {
    reqwest::Method::from_bytes(b"PROPFIND").expect("PROPFIND is a valid HTTP method")
}

/// 列出目录（PROPFIND Depth 1），返回目录内所有项
fn list_dir(client: &Client, headers: &HeaderMap, url: &str) -> Result<Vec<WebDavFile>, String> {
    let body = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<d:propfind xmlns:d=\"DAV:\"><d:allprop/></d:propfind>";
    let resp = client
        .request(propfind_method(), url)
        .headers(headers.clone())
        .header("Depth", "1")
        .header(CONTENT_TYPE, "application/xml; charset=utf-8")
        .header(ACCEPT, "application/xml")
        .body(body)
        .send()
        .map_err(|e| format!("PROPFIND 请求失败: {e}"))?;

    let status = resp.status().as_u16();
    if !(200..400).contains(&status) {
        return Err(format!("PROPFIND 失败，状态码 {status}"));
    }

    let text = resp.text().map_err(|e| format!("读取响应失败: {e}"))?;
    parse_propfind_response(&text, url)
}

/// 解析 PROPFIND 响应（纯函数，便于测试）
fn parse_propfind_response(text: &str, url: &str) -> Result<Vec<WebDavFile>, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("解析响应失败: {e}"))?;

    let mut files = Vec::new();
    // descendants 保持文档顺序（深度优先），避免栈导致的逆序
    for node in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "response")
    {

        // <d:href> 路径
        let mut href: Option<String> = None;
        let mut is_dir = false;
        let mut modified: Option<i64> = None;
        let mut size: Option<u64> = None;

        for child in node.children() {
            if !child.is_element() {
                continue;
            }
            let tag = child.tag_name().name();
            match tag {
                "href" => href = Some(child.text().unwrap_or("").trim().to_string()),
                "propstat" => {
                    for prop in child.descendants().filter(|n| n.is_element()) {
                        let name = prop.tag_name().name();
                        if name == "collection" {
                            is_dir = true;
                        } else if name == "getlastmodified" {
                            if let Some(t) = prop.text() {
                                modified = parse_http_date(t.trim());
                            }
                        } else if name == "getcontentlength" {
                            if let Some(t) = prop.text() {
                                size = t.trim().parse::<u64>().ok();
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let Some(href) = href else { continue };
        if href.is_empty() {
            continue;
        }
        let full_url = join_url(url, &href);
        // 跳过自身目录（href 可能是绝对路径形式）
        if full_url.trim_end_matches('/') == url.trim_end_matches('/') {
            continue;
        }

        let clean = href.trim_end_matches('/');
        let name = percent_decode(clean.rsplit('/').next().unwrap_or(clean));

        files.push(WebDavFile {
            url: full_url,
            name,
            is_directory: is_dir,
            modified,
            size,
        });
    }

    Ok(files)
}

/// 百分号解码（仅处理 %XX，用于文件名展示；URL 本身保留编码形式）
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn join_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if href.starts_with('/') {
        // 绝对路径: 与服务器根拼接
        let target = parse_target_str(base);
        return format!("{}{}", target.0, href);
    }
    // 相对路径
    format!("{}/{}", base.trim_end_matches('/'), href.trim_start_matches('/'))
}

fn parse_target_str(url: &str) -> (String, String) {
    match url::Url::parse(url) {
        Ok(u) => (
            format!("{}://{}{}", u.scheme(), u.host_str().unwrap_or(""), port_suffix(&u)),
            u.path().to_string(),
        ),
        Err(_) => (url.to_string(), String::new()),
    }
}

fn parse_http_date(s: &str) -> Option<i64> {
    // RFC 1123: "Mon, 02 Jan 2006 15:04:05 GMT"
    chrono::DateTime::parse_from_rfc2822(s)
        .ok()
        .map(|dt| dt.timestamp())
}

/// 递归扫描并返回所有音频文件（限制深度避免循环）。
/// 阻塞逻辑必须在阻塞线程执行，否则 tokio 会 panic。
/// `progress`: 可选进度回调通道（已处理数, 总数）。
/// `read_tags`: true=逐首探测标签；false=快速扫描（仅文件名）。
pub async fn fetch_all_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<ProgressSender<(usize, usize)>>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let cfg = config.clone();
    let cache = cover_cache.cloned();
    let existing = existing_modified.cloned();
    tauri::async_runtime::spawn_blocking(move || {
        fetch_all_songs_blocking(&cfg, cache.as_ref(), existing.as_ref(), progress, read_tags)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn fetch_all_songs_blocking(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<ProgressSender<(usize, usize)>>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let root = effective_initial_path(config);
    let base = parse_target(config).clean_base_url;
    let root_url = format!("{}{}", base, root);

    scan_from(
        &client,
        &headers,
        config,
        &root_url,
        cover_cache,
        existing_modified,
        progress,
        read_tags,
    )
}

/// 从指定目录 URL 递归扫描（文件夹浏览“加入音乐库”用，默认快速模式）
pub fn scan_dir(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    dir_url: &str,
    progress: Option<ProgressSender<(usize, usize)>>,
) -> Result<Vec<ScannedSong>, String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let root = dir_url.trim_end_matches('/');
    scan_from(
        &client,
        &headers,
        config,
        root,
        cover_cache,
        existing_modified,
        progress,
        false,
    )
}

/// 核心扫描：BFS 列目录 + （可选）并行读取元数据
fn scan_from(
    client: &Client,
    headers: &HeaderMap,
    config: &StreamServerConfig,
    root_url: &str,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<ProgressSender<(usize, usize)>>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    // BFS 扫描目录
    let mut queue = VecDeque::new();
    queue.push_back(root_url.to_string());

    let mut audio_urls: Vec<(String, Option<i64>)> = Vec::new();

    while let Some(dir) = queue.pop_front() {
        let entries = match list_dir(client, headers, &dir) {
            Ok(list) => list,
            Err(e) => {
                log::warn!("[WebDAV] 扫描目录失败 {dir}: {e}");
                Vec::new()
            }
        };
        for entry in entries {
            if entry.is_directory {
                queue.push_back(entry.url);
            } else {
                let ext = entry
                    .name
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                if AUDIO_EXTS.contains(&ext.as_str()) {
                    audio_urls.push((entry.url, entry.modified));
                }
            }
        }
    }

    // 快速模式：仅文件名入库，不下载任何音频数据（慢网速下秒级完成）
    if !read_tags {
        let songs: Vec<ScannedSong> = audio_urls
            .iter()
            .map(|(url, modified)| fallback_song_with_modified(url, *modified))
            .collect();
        return Ok(songs);
    }

    // 完整模式：并行读取元数据（复用同一 HTTP 连接池，避免慢网速下重复 TLS 握手）
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .map_err(|e| e.to_string())?;

    let total = audio_urls.len();
    let done = AtomicUsize::new(0);

    let mut songs: Vec<ScannedSong> = pool.install(|| {
        audio_urls
            .par_iter()
            .map(|(url, modified)| {
                let song = read_remote_song(client, headers, config, url, *modified, cover_cache, existing_modified);
                if let Some(tx) = &progress {
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if n % 10 == 0 || n == total {
                        let _ = tx.send((n, total));
                    }
                }
                song
            })
            .collect()
    });

    songs.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    Ok(songs)
}

/// 单首歌探测标签（后台补全用）。仅当探测成功且有时长时返回 Some。
pub fn probe_song(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    url: &str,
) -> Option<ScannedSong> {
    let client = build_client().ok()?;
    let headers = build_auth_headers(config);
    let song = read_remote_song(&client, &headers, config, url, None, cover_cache, None);
    (song.duration > 0.0).then_some(song)
}

/// 读取远程歌曲元数据：下载文件头部到临时文件后用 lofty 解析。
/// 用 `Read::take` 限制读取量，即使服务器忽略 Range 也不会全量下载大文件。
/// 若文件修改时间未变（增量同步），跳过下载直接返回文件名级数据。
fn read_remote_song(
    client: &Client,
    headers: &HeaderMap,
    _config: &StreamServerConfig,
    url: &str,
    modified: Option<i64>,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
) -> ScannedSong {
    // 增量同步：文件未变化时跳过网络探测
    if let (Some(modified), Some(existing)) = (modified, existing_modified) {
        if existing.get(url) == Some(&modified) {
            return fallback_song_with_modified(url, Some(modified));
        }
    }

    use std::io::Read;

    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("bayin-probe-{:x}.bin", hash_url(url)));
    let mut tmp_file = match std::fs::File::create(&tmp_path) {
        Ok(f) => f,
        Err(_) => return fallback_song(url),
    };

    let resp = match client
        .get(url)
        .headers(headers.clone())
        .header("Range", format!("bytes=0-{}", METADATA_PROBE_BYTES - 1))
        .send()
    {
        Ok(r) => r,
        Err(_) => return fallback_song(url),
    };
    let status = resp.status().as_u16();

    let ok = if status == 200 || status == 206 {
        let mut limited = resp.take(METADATA_PROBE_BYTES);
        std::io::copy(&mut limited, &mut tmp_file).is_ok()
    } else {
        false
    };
    drop(tmp_file);

    let song = if ok {
        parse_probe_file(&tmp_path, url, modified, cover_cache)
    } else {
        fallback_song_with_modified(url, modified)
    };

    let _ = std::fs::remove_file(&tmp_path);
    song
}

fn hash_url(url: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    h.finish()
}

/// 用 lofty 解析探测文件
fn parse_probe_file(
    path: &std::path::Path,
    url: &str,
    modified: Option<i64>,
    cover_cache: Option<&CoverCache>,
) -> ScannedSong {
    let fallback = fallback_song(url);

    let tagged_file = match Probe::open(path) {
        Ok(p) => match p.read() {
            Ok(f) => f,
            Err(_) => return fallback,
        },
        Err(_) => return fallback,
    };

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback_title(url));

    let artist = tag
        .and_then(|t| t.artist().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Album".to_string());

    let properties = tagged_file.properties();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_sq = matches!(ext.as_str(), "flac" | "wav" | "ape" | "wv" | "aiff" | "alac");
    let is_hr = properties.sample_rate().map(|r| r > 44100).unwrap_or(false)
        || properties.bit_depth().map(|d| d > 16).unwrap_or(false);

    // 提取封面并缓存到本地（失败时忽略，不影响歌曲本身）
    let cover_hash = cover_cache.and_then(|cache| {
        tag
            .and_then(|t| t.pictures().first().cloned())
            .and_then(|pic| {
                cache
                    .save_cover(pic.data(), pic.mime_type().map(|m| m.as_str()))
                    .ok()
            })
    });

    ScannedSong {
        id: url.to_string(),
        title,
        artist,
        album,
        duration: properties.duration().as_secs_f64(),
        file_path: url.to_string(),
        file_size: 0,
        cover_url: None,
        cover_hash,
        file_modified: modified,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
        format: Some(ext),
        bit_depth: properties.bit_depth(),
        sample_rate: properties.sample_rate(),
        bitrate: properties.audio_bitrate(),
        channels: properties.channels(),
        created_at: None,
    }
}

fn fallback_title(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = name.rsplit('.').next().unwrap_or(name);
    name.to_string()
}

fn fallback_song(url: &str) -> ScannedSong {
    fallback_song_with_modified(url, None)
}

/// 文件名级兜底歌曲（增量同步跳过探测时使用），保留远程修改时间
fn fallback_song_with_modified(url: &str, modified: Option<i64>) -> ScannedSong {
    let ext = url
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    ScannedSong {
        id: url.to_string(),
        title: fallback_title(url),
        artist: "Unknown Artist".to_string(),
        album: "Unknown Album".to_string(),
        duration: 0.0,
        file_path: url.to_string(),
        file_size: 0,
        cover_url: None,
        cover_hash: None,
        file_modified: modified,
        is_hr: None,
        is_sq: None,
        format: Some(ext),
        bit_depth: None,
        sample_rate: None,
        bitrate: None,
        channels: None,
        created_at: None,
    }
}

/// 构造播放时的请求头（Basic Auth）
pub fn stream_headers(config: &StreamServerConfig) -> Vec<(String, String)> {
    let token = BASE64.encode(format!("{}:{}", config.username, config.password));
    vec![("Authorization".to_string(), format!("Basic {token}"))]
}

/// 播放 URL：song_id 即完整 URL
pub fn stream_url(_config: &StreamServerConfig, song_id: &str) -> String {
    song_id.to_string()
}

/// 由歌曲 URL 推导同名 .lrc 侧车文件 URL（保留查询参数）
fn lrc_sidecar_url(song_url: &str) -> String {
    let (base, query) = match song_url.find('?') {
        Some(i) => (&song_url[..i], Some(&song_url[i..])),
        None => (song_url, None),
    };
    let last_slash = base.rfind('/').unwrap_or(0);
    let lrc_base = match base.rfind('.') {
        Some(i) if i > last_slash => format!("{}.lrc", &base[..i]),
        _ => format!("{base}.lrc"),
    };
    match query {
        Some(q) => format!("{lrc_base}{q}"),
        None => lrc_base,
    }
}

/// 读取 WebDAV 歌曲的侧车 .lrc 歌词（同目录同名文件）
pub fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let client = build_client().ok()?;
    let headers = build_auth_headers(config);
    let url = lrc_sidecar_url(song_id);

    let resp = client.get(&url).headers(headers).send().ok()?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return None;
    }
    let text = resp.text().ok()?;
    let trimmed = text.trim_start();
    // 过滤空文件或 HTML 错误页
    if trimmed.is_empty()
        || trimmed.starts_with("<!DOCTYPE")
        || trimmed.starts_with("<html")
    {
        return None;
    }
    Some(text)
}

/// WebDAV 缓存统计
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavCacheStats {
    pub file_count: usize,
    pub total_size: u64,
}

/// 统计 WebDAV 歌曲本地缓存占用
pub fn cache_stats(cache_root: &std::path::Path) -> WebDavCacheStats {
    let dir = cache_root.join("webdav");
    let mut stats = WebDavCacheStats::default();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    stats.file_count += 1;
                    stats.total_size += meta.len();
                }
            }
        }
    }
    stats
}

/// 清理 WebDAV 歌曲本地缓存，返回删除的文件数
pub fn clear_cache(cache_root: &std::path::Path) -> usize {
    let dir = cache_root.join("webdav");
    let mut removed = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// 删除远程文件或文件夹（DELETE 请求，文件夹由服务器递归处理）
pub fn delete(config: &StreamServerConfig, url: &str) -> Result<(), String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let resp = client
        .delete(url)
        .headers(headers)
        .send()
        .map_err(|e| format!("删除请求失败: {e}"))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) || status == 404 {
        // 404 视为已删除
        Ok(())
    } else {
        Err(format!("删除失败，状态码 {status}"))
    }
}

/// 移动/重命名远程文件或文件夹（MOVE 请求）
pub fn move_entry(
    config: &StreamServerConfig,
    source: &str,
    destination: &str,
) -> Result<(), String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let resp = client
        .request(reqwest::Method::from_bytes(b"MOVE").expect("MOVE is a valid HTTP method"), source)
        .headers(headers)
        .header("Destination", destination)
        .send()
        .map_err(|e| format!("移动请求失败: {e}"))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("移动失败，状态码 {status}"))
    }
}

/// 上传本地文件到远程目录（PUT 请求）
pub fn upload(
    config: &StreamServerConfig,
    local_path: &str,
    remote_url: &str,
) -> Result<(), String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let file = std::fs::File::open(local_path).map_err(|e| format!("打开文件失败: {e}"))?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();

    let resp = client
        .put(remote_url)
        .headers(headers)
        .header(reqwest::header::CONTENT_LENGTH, length)
        .body(file)
        .send()
        .map_err(|e| format!("上传请求失败: {e}"))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("上传失败，状态码 {status}"))
    }
}

/// 目录浏览项（前端展示用）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavEntry {
    pub name: String,
    pub url: String,
    pub is_directory: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
}

/// 列出目录内容（供文件夹浏览）。
/// `dir_url`: 目标目录完整 URL；None 表示服务器的初始目录（basePath）。
pub fn list_dir_entries(
    config: &StreamServerConfig,
    dir_url: Option<&str>,
) -> Result<Vec<WebDavEntry>, String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let base = parse_target(config).clean_base_url;
    let url = match dir_url {
        Some(u) if !u.trim().is_empty() => u.trim_end_matches('/').to_string(),
        _ => format!("{}{}", base, effective_initial_path(config)),
    };

    let files = list_dir(&client, &headers, &url)?;
    let mut entries: Vec<WebDavEntry> = files
        .into_iter()
        .map(|f| WebDavEntry {
            name: f.name,
            url: f.url,
            is_directory: f.is_directory,
            size: f.size,
            modified: f.modified,
        })
        .collect();
    // 文件夹在前，其余按名称排序
    entries.sort_by(|a, b| b.is_directory.cmp(&a.is_directory).then(a.name.cmp(&b.name)));
    Ok(entries)
}

/// 计算歌曲缓存文件名（基于 server_id + URL 的稳定 hash）
fn cache_file_name(server_id: &str, song_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(server_id.as_bytes());
    hasher.update(b":");
    hasher.update(song_id.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    let short = &hex[..24];
    // 保留扩展名，方便识别格式
    let ext = song_id
        .split('?')
        .next()
        .unwrap_or(song_id)
        .rsplit('.')
        .next()
        .unwrap_or("bin");
    let ext = if ext.contains('/') || ext.is_empty() { "bin" } else { ext };
    format!("{short}.{ext}")
}

/// WebDAV 歌曲的本地缓存路径（不存在时返回 None）
pub fn cached_path(cache_root: &std::path::Path, server_id: &str, song_id: &str) -> std::path::PathBuf {
    cache_root.join("webdav").join(cache_file_name(server_id, song_id))
}

/// 是否已有本地缓存（供前端缓存命中判断）
pub fn has_cached_song(cache_root: &std::path::Path, server_id: &str, song_id: &str) -> bool {
    cached_path(cache_root, server_id, song_id).exists()
}

/// 下载歌曲完整文件到本地缓存，返回缓存路径。
/// 已有缓存时直接返回，避免重复下载。
pub fn cache_song(
    config: &StreamServerConfig,
    cache_root: &std::path::Path,
    server_id: &str,
    song_id: &str,
) -> Result<String, String> {
    let dir = cache_root.join("webdav");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建缓存目录: {e}"))?;
    let path = cached_path(cache_root, server_id, song_id);
    if path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(path.to_string_lossy().to_string());
    }

    let client = build_client()?;
    let headers = build_auth_headers(config);
    let resp = client
        .get(song_id)
        .headers(headers)
        .send()
        .map_err(|e| format!("下载失败: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return Err(format!("下载失败，状态码 {status}"));
    }

    // 写入临时文件再重命名，避免半成品缓存
    let tmp_path = dir.join(format!("{}.part", cache_file_name(server_id, song_id)));
    let mut out = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
    {
        use std::io::Read;
        std::io::copy(&mut resp.take(u64::MAX), &mut out)
            .map_err(|e| format!("写入缓存失败: {e}"))?;
    }
    drop(out);
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("缓存完成失败: {e}"))?;

    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PROPFIND: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/dav/music/</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype><D:collection/></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/music/Album%20One/</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype><D:collection/></D:resourcetype>
        <D:getlastmodified>Tue, 15 Jun 2021 08:00:00 GMT</D:getlastmodified>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/music/track%201.mp3</D:href>
    <D:propstat>
      <D:prop>
        <D:getlastmodified>Mon, 14 Jun 2021 09:30:00 GMT</D:getlastmodified>
        <D:getcontentlength>1234567</D:getcontentlength>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

    #[test]
    fn parses_propfind_response() {
        let files = parse_propfind_response(SAMPLE_PROPFIND, "https://host/dav/music/").unwrap();

        // 2 个子项（跳过自身 /dav/music/）
        assert_eq!(files.len(), 2);

        let dir = &files[0];
        assert!(dir.is_directory);
        assert_eq!(dir.name, "Album One");
        assert_eq!(dir.url, "https://host/dav/music/Album%20One/");

        let song = &files[1];
        assert!(!song.is_directory);
        assert_eq!(song.name, "track 1.mp3");
        assert_eq!(song.url, "https://host/dav/music/track%201.mp3");
        assert!(song.modified.is_some());
    }

    #[test]
    fn skips_self_response() {
        // 只有自身的响应时返回空
        let files =
            parse_propfind_response(SAMPLE_PROPFIND, "https://host/dav/music/").unwrap();
        assert!(!files.iter().any(|f| f.url == "https://host/dav/music/"));
    }

    #[test]
    fn parses_target_url() {
        let t = parse_target(&StreamServerConfig {
            server_type: crate::models::ServerType::Webdav,
            server_name: "t".to_string(),
            server_url: "https://host:8080/dav/music".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        });
        assert_eq!(t.clean_base_url, "https://host:8080");
        assert_eq!(t.initial_path, "/dav/music");
    }

    #[test]
    fn base_path_overrides_url_path() {
        let cfg = StreamServerConfig {
            server_type: crate::models::ServerType::Webdav,
            server_name: "t".to_string(),
            server_url: "https://host/dav/music".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: Some("/remote.php/dav/files/user/音乐".to_string()),
            access_token: None,
            user_id: None,
        };
        assert_eq!(effective_initial_path(&cfg), "/remote.php/dav/files/user/音乐");
    }

    #[test]
    fn builds_basic_auth_header() {
        let cfg = StreamServerConfig {
            server_type: crate::models::ServerType::Webdav,
            server_name: "t".to_string(),
            server_url: "https://host".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        };
        let h = build_auth_headers(&cfg);
        let auth = h.get("authorization").unwrap().to_str().unwrap();
        assert_eq!(auth, "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn stream_headers_are_basic_auth() {
        let cfg = StreamServerConfig {
            server_type: crate::models::ServerType::Webdav,
            server_name: "t".to_string(),
            server_url: "https://host".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        };
        let headers = stream_headers(&cfg);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Authorization");
        assert_eq!(headers[0].1, "Basic dXNlcjpwYXNz");
    }
}

#[cfg(test)]
mod lyrics_tests {
    use super::*;

    #[test]
    fn sidecar_url_replaces_extension() {
        assert_eq!(
            lrc_sidecar_url("https://host/dav/music/Album/song.mp3"),
            "https://host/dav/music/Album/song.lrc"
        );
    }

    #[test]
    fn sidecar_url_handles_query_and_encoded_names() {
        assert_eq!(
            lrc_sidecar_url("https://host/dav/track%201.flac?token=abc"),
            "https://host/dav/track%201.lrc?token=abc"
        );
    }

    #[test]
    fn sidecar_url_without_dot_in_filename() {
        assert_eq!(
            lrc_sidecar_url("https://host/dav/noext"),
            "https://host/dav/noext.lrc"
        );
    }

    #[test]
    fn sidecar_url_ignores_dot_in_directory() {
        assert_eq!(
            lrc_sidecar_url("https://host/dav/music.v2/song"),
            "https://host/dav/music.v2/song.lrc"
        );
    }
}
