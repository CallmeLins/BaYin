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
use std::collections::VecDeque;
use std::time::Duration;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};

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

/// PROPFIND 深度 0，测试连接是否可用
pub async fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
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

/// 递归扫描并返回所有音频文件（限制深度避免循环）
pub async fn fetch_all_songs(config: &StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    let client = build_client()?;
    let headers = build_auth_headers(config);
    let root = effective_initial_path(config);
    let base = parse_target(config).clean_base_url;
    let root_url = format!("{}{}", base, root);

    // BFS 扫描目录
    let mut queue = VecDeque::new();
    queue.push_back(root_url.clone());

    let mut audio_urls: Vec<(String, Option<i64>)> = Vec::new();

    while let Some(dir) = queue.pop_front() {
        let entries = match list_dir(&client, &headers, &dir) {
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

    // 并行读取元数据
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .map_err(|e| e.to_string())?;

    let mut songs: Vec<ScannedSong> = pool.install(|| {
        audio_urls
            .par_iter()
            .map(|(url, modified)| read_remote_song(config, url, *modified))
            .collect()
    });

    songs.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    Ok(songs)
}

/// 读取远程歌曲元数据：下载文件头部到临时文件后用 lofty 解析。
/// 用 `Read::take` 限制读取量，即使服务器忽略 Range 也不会全量下载大文件。
fn read_remote_song(config: &StreamServerConfig, url: &str, _modified: Option<i64>) -> ScannedSong {
    use std::io::Read;

    let client = match build_client() {
        Ok(c) => c,
        Err(_) => return fallback_song(url),
    };
    let headers = build_auth_headers(config);

    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("bayin-probe-{:x}.bin", hash_url(url)));
    let mut tmp_file = match std::fs::File::create(&tmp_path) {
        Ok(f) => f,
        Err(_) => return fallback_song(url),
    };

    let resp = match client
        .get(url)
        .headers(headers)
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
        parse_probe_file(&tmp_path, url)
    } else {
        fallback_song(url)
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
fn parse_probe_file(path: &std::path::Path, url: &str) -> ScannedSong {
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

    ScannedSong {
        id: url.to_string(),
        title,
        artist,
        album,
        duration: properties.duration().as_secs_f64(),
        file_path: url.to_string(),
        file_size: 0,
        cover_url: None,
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
