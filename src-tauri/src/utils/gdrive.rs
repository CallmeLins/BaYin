//! Google Drive（阶段 D5-1）。
//!
//! ## 认证模型
//!
//! 标准 OAuth2。配置三样：
//!
//! | 配置字段 | 含义 |
//! |---|---|
//! | `username` | `client_id` |
//! | `password` | `client_secret` |
//! | `access_token` | 刷新令牌（`refresh_token`） |
//! | `base_path` | 起始文件夹 id，默认 `root` |
//!
//! 刷新令牌可在 Google OAuth Playground 或自建授权页拿一次，长期有效。
//!
//! ## 协议要点
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 刷新令牌 | `POST https://oauth2.googleapis.com/token`，form |
//! | 列目录 | `GET https://www.googleapis.com/drive/v3/files?q=&pageSize=1000&pageToken=&fields=` |
//! | 下载 | `GET https://www.googleapis.com/drive/v3/files/{id}?alt=media` |
//!
//! ## 注意
//!
//! `alt=media` 的地址**必须带 `Authorization: Bearer`**，且支持 Range。
//! 所以播放走 `get_stream_play_info` 把令牌头一起交给播放引擎。
//! 令牌有效期只有 1 小时，每次取直链时都用当前最新令牌，不缓存 URL。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::build_blocking_client_for;

const API_BASE: &str = "https://www.googleapis.com/drive/v3";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// 曲目引用前缀：`/gdrive/{file_id}.{ext}`。
const REF_PREFIX: &str = "/gdrive/";
const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 20_000;
const MAX_DIRS: usize = 5_000;
const REFRESH_SKEW_SECS: i64 = 120;
const DEFAULT_UA: &str = "Mozilla/5.0 BaYin/1.0";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

// ==================== 曲目引用 ====================

/// 合成引用：`/gdrive/{file_id}.{ext}`。
pub fn track_path(file_id: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!(
        "{REF_PREFIX}{file_id}.{}",
        if ext.is_empty() { "bin" } else { &ext }
    )
}

/// 从合成引用反解 `file_id`（Google 的 id 含 `-`/`_`，不含点）。
pub fn track_file_id(path: &str) -> Option<String> {
    let rest = path.strip_prefix(REF_PREFIX)?;
    let name = rest.rsplit('/').next().unwrap_or(rest);
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => stem,
        _ => name,
    };
    (!stem.is_empty()
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then(|| stem.to_string())
}

// ==================== 令牌 ====================

#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: i64,
}

fn token_cache() -> &'static Mutex<HashMap<String, CachedToken>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedToken>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn refresh_token_of(config: &StreamServerConfig) -> Option<String> {
    let raw = config.access_token.as_deref().unwrap_or("").trim();
    (!raw.is_empty()).then(|| raw.to_string())
}

fn client_credentials(config: &StreamServerConfig) -> Option<(String, String)> {
    let id = config.username.trim();
    let secret = config.password.trim();
    (!id.is_empty() && !secret.is_empty()).then(|| (id.to_string(), secret.to_string()))
}

fn client() -> Result<reqwest::blocking::Client, String> {
    build_blocking_client_for(API_BASE)
}

/// 取得可用 access_token。`force_refresh` 用于 401 后强制换新。
pub fn current_access_token(
    config: &StreamServerConfig,
    force_refresh: bool,
) -> Result<String, String> {
    let refresh = refresh_token_of(config);

    if !force_refresh {
        if let Some(key) = refresh.as_ref() {
            if let Ok(cache) = token_cache().lock() {
                if let Some(cached) = cache.get(key) {
                    if cached.expires_at > now_secs() + REFRESH_SKEW_SECS {
                        return Ok(cached.access_token.clone());
                    }
                }
            }
        }
    }

    let (client_id, client_secret) = client_credentials(config).ok_or_else(|| {
        "Google Drive：请填写 client_id 与 client_secret".to_string()
    })?;
    let refresh = refresh.ok_or_else(|| "Google Drive：请填写刷新令牌".to_string())?;

    let response = client()?
        .post(TOKEN_URL)
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
        ])
        .send()
        .map_err(|e| format!("Google Drive 刷新令牌请求失败: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| format!("Google Drive 刷新令牌响应不是 JSON（HTTP {status}）: {e}"))?;

    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            let detail = body
                .get("error_description")
                .or_else(|| body.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| body.to_string());
            format!("Google Drive 刷新令牌失败: {detail}")
        })?
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(3600);

    let token = CachedToken {
        access_token,
        expires_at: now_secs() + expires_in,
    };
    if let Ok(mut cache) = token_cache().lock() {
        cache.insert(refresh, token.clone());
    }
    Ok(token.access_token)
}

// ==================== HTTP ====================

#[derive(Debug)]
enum GError {
    Unauthorized,
    Failed(String),
}

impl GError {
    fn message(self) -> String {
        match self {
            GError::Unauthorized => "Google Drive：令牌已失效，且刷新失败".to_string(),
            GError::Failed(message) => message,
        }
    }
}

fn error_message(body: &Value) -> String {
    body.get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .or_else(|| body.get("error_description").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string())
}

fn parse_response(response: reqwest::blocking::Response) -> Result<Value, GError> {
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| GError::Failed(format!("Google Drive 响应不是 JSON（HTTP {status}）: {e}")))?;

    if status.as_u16() == 401 {
        return Err(GError::Unauthorized);
    }
    if !status.is_success() {
        // Google 用 400 + error.status=UNAUTHENTICATED 表达令牌问题。
        let unauthorized = body
            .get("error")
            .and_then(|e| e.get("status"))
            .and_then(Value::as_str)
            .map(|s| s == "UNAUTHENTICATED")
            .unwrap_or(false);
        if unauthorized {
            return Err(GError::Unauthorized);
        }
        return Err(GError::Failed(format!(
            "Google Drive HTTP {status}: {}",
            error_message(&body)
        )));
    }
    Ok(body)
}

fn get_json(
    client: &reqwest::blocking::Client,
    url: &str,
    access_token: &str,
) -> Result<Value, GError> {
    let response = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {access_token}"))
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .send()
        .map_err(|e| GError::Failed(format!("Google Drive 请求失败: {e}")))?;
    parse_response(response)
}

fn with_token_retry<T>(
    config: &StreamServerConfig,
    call: impl Fn(&str) -> Result<T, GError>,
) -> Result<T, String> {
    let token = current_access_token(config, false)?;
    match call(&token) {
        Ok(value) => Ok(value),
        Err(GError::Unauthorized) => {
            let token = current_access_token(config, true)?;
            call(&token).map_err(GError::message)
        }
        Err(error) => Err(error.message()),
    }
}

// ==================== 响应解析 ====================

fn value_to_i64(value: &Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq)]
struct RemoteItem {
    file_id: String,
    name: String,
    size: u64,
    is_dir: bool,
    md5: Option<String>,
    modified: Option<i64>,
}

fn parse_item(value: &Value) -> Option<RemoteItem> {
    let name = value.get("name").and_then(Value::as_str)?.to_string();
    let file_id = value.get("id").and_then(Value::as_str)?.to_string();
    if file_id.is_empty() {
        return None;
    }
    let mime = value.get("mimeType").and_then(Value::as_str).unwrap_or("");

    Some(RemoteItem {
        file_id,
        name,
        size: value.get("size").map(value_to_i64).unwrap_or(0).max(0) as u64,
        is_dir: mime == FOLDER_MIME,
        md5: value
            .get("md5Checksum")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        modified: value
            .get("modifiedTime")
            .and_then(Value::as_str)
            .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
            .map(|dt| dt.timestamp()),
    })
}

struct ListPage {
    items: Vec<RemoteItem>,
    next_page_token: Option<String>,
}

fn parse_list_page(body: &Value) -> Result<ListPage, GError> {
    let files = body
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| GError::Failed("Google Drive 目录响应缺少 files".to_string()))?;
    Ok(ListPage {
        items: files.iter().filter_map(parse_item).collect(),
        next_page_token: body
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string),
    })
}

// ==================== 列表 / 扫描 ====================

fn root_folder_id(config: &StreamServerConfig) -> String {
    let raw = config
        .base_path
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .trim_matches('/');
    if raw.is_empty() {
        "root".to_string()
    } else {
        raw.to_string()
    }
}

/// Google 的 `q` 语法：`'<folderId>' in parents and trashed = false`。
fn list_query(folder_id: &str) -> String {
    format!("'{folder_id}' in parents and trashed = false")
}

fn list_page(
    client: &reqwest::blocking::Client,
    token: &str,
    folder_id: &str,
    page_token: Option<&str>,
) -> Result<ListPage, GError> {
    let fields = "nextPageToken,files(id,name,mimeType,size,modifiedTime,md5Checksum)";
    let mut url = format!(
        "{API_BASE}/files?q={}&pageSize={PAGE_SIZE}&orderBy=name&fields={}&supportsAllDrives=true&includeItemsFromAllDrives=true",
        urlencode(&list_query(folder_id)),
        urlencode(fields)
    );
    if let Some(token) = page_token {
        url.push_str(&format!("&pageToken={}", urlencode(token)));
    }
    let body = get_json(client, &url, token)?;
    parse_list_page(&body)
}

fn list_page_with_retry(
    config: &StreamServerConfig,
    client: &reqwest::blocking::Client,
    folder_id: &str,
    page_token: Option<&str>,
) -> Result<ListPage, String> {
    with_token_retry(config, |token| {
        list_page(client, token, folder_id, page_token)
    })
}

/// 极简百分号编码：只保留 unreserved 字符。
fn urlencode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() * 3);
    for byte in raw.as_bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(*byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// 递归扫描曲库（文件夹 BFS，按 id）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    _cover_cache: Option<&crate::utils::cover::CoverCache>,
    _existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    _read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let client = client()?;

    let mut queue: VecDeque<String> = VecDeque::from([root_folder_id(config)]);
    let mut visited_dirs: HashSet<String> = HashSet::new();
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut songs: Vec<ScannedSong> = Vec::new();
    let mut dir_count = 0usize;

    while let Some(folder_id) = queue.pop_front() {
        if !visited_dirs.insert(folder_id.clone()) {
            continue;
        }
        dir_count += 1;
        if dir_count > MAX_DIRS {
            return Err("Google Drive 目录数量超过安全上限，请检查起始文件夹设置".to_string());
        }

        let mut page_token: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let page =
                list_page_with_retry(config, &client, &folder_id, page_token.as_deref())?;

            for item in &page.items {
                if item.is_dir {
                    queue.push_back(item.file_id.clone());
                    continue;
                }

                let path = Path::new(&item.name);
                let Some(raw_ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                let Some(ext) = normalize_audio_extension(raw_ext) else {
                    log::debug!("[GoogleDrive] 跳过无可用格式的文件: {} (.{raw_ext})", item.name);
                    continue;
                };
                if !seen_files.insert(item.file_id.clone()) {
                    continue;
                }

                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&item.name)
                    .to_string();
                let reference = track_path(&item.file_id, &ext);

                songs.push(ScannedSong {
                    id: reference.clone(),
                    title,
                    artist: String::new(),
                    album: String::new(),
                    duration: 0.0,
                    file_path: reference,
                    file_size: item.size,
                    cover_url: None,
                    cover_hash: None,
                    file_modified: item.modified,
                    is_hr: None,
                    is_sq: Some(is_lossless_extension(&ext)),
                    format: Some(ext.to_uppercase()),
                    bit_depth: None,
                    sample_rate: None,
                    bitrate: None,
                    channels: None,
                    created_at: item.modified,
                });

                let _ = &item.md5; // 预留：增量扫描可用 md5Checksum 作 revision
            }

            if let Some(tx) = &progress {
                let count = songs.len();
                let _ = tx.send((count, count.max(1)));
            }

            match page.next_page_token {
                Some(next) => {
                    if Some(next.as_str()) == page_token.as_deref() {
                        break;
                    }
                    page_token = Some(next);
                }
                None => break,
            }
        }
    }

    if let Some(tx) = &progress {
        let total = songs.len();
        let _ = tx.send((total, total));
    }
    Ok(songs)
}

// ==================== 连接测试 ====================

fn fetch_about(config: &StreamServerConfig) -> Result<(String, String), String> {
    let client = client()?;
    let body = with_token_retry(config, |token| {
        get_json(
            &client,
            &format!("{API_BASE}/about?fields=user(displayName,emailAddress)&supportsAllDrives=true"),
            token,
        )
    })?;
    let user = body.get("user").cloned().unwrap_or(Value::Null);
    let name = user
        .get("displayName")
        .and_then(Value::as_str)
        .unwrap_or("Google 用户")
        .to_string();
    let email = user
        .get("emailAddress")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok((name, email))
}

pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if client_credentials(config).is_none() || refresh_token_of(config).is_none() {
        return ConnectionTestResult {
            success: false,
            message: "请填写 Google Drive 的 client_id、client_secret 与刷新令牌".to_string(),
            server_version: None,
        };
    }
    match fetch_about(config) {
        Ok((name, email)) => ConnectionTestResult {
            success: true,
            message: if email.is_empty() {
                format!("已连接到 Google Drive（{name}）")
            } else {
                format!("已连接到 Google Drive（{name} / {email}）")
            },
            server_version: None,
        },
        Err(error) => ConnectionTestResult {
            success: false,
            message: error,
            server_version: None,
        },
    }
}

// ==================== 播放 ====================

/// 媒体直链 + 当前令牌（`alt=media` 必须带 Bearer）。
pub fn stream_url_and_headers(
    config: &StreamServerConfig,
    song_id: &str,
) -> (String, Vec<(String, String)>) {
    let Some(file_id) = track_file_id(song_id) else {
        log::error!("[GoogleDrive] 无法识别的曲目引用: {song_id}");
        return (String::new(), Vec::new());
    };
    let token = match current_access_token(config, false) {
        Ok(token) => token,
        Err(error) => {
            log::error!("[GoogleDrive] 获取令牌失败（{file_id}）: {error}");
            return (String::new(), Vec::new());
        }
    };
    let url = format!(
        "{API_BASE}/files/{}?alt=media&supportsAllDrives=true",
        urlencode(&file_id)
    );
    (
        url,
        vec![
            ("Authorization".to_string(), format!("Bearer {token}")),
            ("User-Agent".to_string(), DEFAULT_UA.to_string()),
        ],
    )
}

/// 只返回 URL 时无法携带令牌，故调用方应优先走 `stream_url_and_headers`。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    stream_url_and_headers(config, song_id).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_reference_round_trips() {
        let path = track_path("1AbC-d_ef", ".FLAC");
        assert_eq!(path, "/gdrive/1AbC-d_ef.flac");
        assert_eq!(track_file_id(&path).as_deref(), Some("1AbC-d_ef"));
        assert!(track_file_id("/aliyundrive/1AbC.flac").is_none());
        assert!(track_file_id("/gdrive/a b.flac").is_none());
    }

    #[test]
    fn parses_folder_and_file_items() {
        let folder = serde_json::json!({
            "id": "f1", "name": "Music", "mimeType": FOLDER_MIME
        });
        let file = serde_json::json!({
            "id": "x1", "name": "song.flac", "mimeType": "audio/flac",
            "size": "8192", "md5Checksum": "abc",
            "modifiedTime": "2026-10-17T02:00:00.000Z"
        });

        let folder = parse_item(&folder).expect("folder");
        assert!(folder.is_dir);

        let file = parse_item(&file).expect("file");
        assert!(!file.is_dir);
        assert_eq!(file.size, 8192);
        assert_eq!(file.md5.as_deref(), Some("abc"));
        assert!(file.modified.is_some());
    }

    #[test]
    fn parses_list_page_token() {
        let body = serde_json::json!({
            "nextPageToken": "tok2",
            "files": [{"id": "a", "name": "x", "mimeType": "audio/mpeg"}]
        });
        let page = parse_list_page(&body).expect("page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.next_page_token.as_deref(), Some("tok2"));

        let last = serde_json::json!({"files": []});
        assert!(parse_list_page(&last).expect("page").next_page_token.is_none());
    }

    #[test]
    fn builds_drive_query() {
        assert_eq!(
            list_query("root"),
            "'root' in parents and trashed = false"
        );
        assert_eq!(
            urlencode(&list_query("root")),
            "%27root%27%20in%20parents%20and%20trashed%20%3D%20false"
        );
    }

    #[test]
    fn defaults_root_folder() {
        let mut config = StreamServerConfig {
            server_type: crate::models::ServerType::GoogleDrive,
            server_name: String::new(),
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        };
        assert_eq!(root_folder_id(&config), "root");
        config.base_path = Some("/abc/".to_string());
        assert_eq!(root_folder_id(&config), "abc");
    }
}
