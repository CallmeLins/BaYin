//! 阿里云盘（开放平台 OpenAPI，阶段 D4）。
//!
//! ## 认证模型
//!
//! 和百度一样只有 OAuth2，但阿里把「刷新令牌」做成了长期凭证（不轮换），
//! 所以配置三样即可：
//!
//! | 配置字段 | 含义 |
//! |---|---|
//! | `username` | `client_id` |
//! | `password` | `client_secret` |
//! | `access_token` | 刷新令牌（`refresh_token`）——运行时换出的 access_token 只进内存 |
//! | `base_path` | 起始目录的 `file_id`，默认 `root` |
//!
//! ## 协议要点（已对照 primuse `AliyunDriveSource` 核实）
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 刷新令牌 | `POST {api}/oauth/access_token`，JSON `{grant_type, refresh_token, client_id, client_secret}` |
//! | 云盘信息 | `POST {api}/adrive/v1.0/user/getDriveInfo` |
//! | 列目录 | `POST {api}/adrive/v1.0/openFile/list`，JSON `{drive_id, parent_file_id, limit, marker, order_by}` |
//! | 下载地址 | `POST {api}/adrive/v1.0/openFile/getDownloadUrl`，JSON `{drive_id, file_id}` |
//!
//! 认证头 `Authorization: Bearer <access_token>`，请求体一律 JSON。
//!
//! ## 目录模型
//!
//! 阿里用**稳定的 `file_id`** 表示目录（根是字符串 `root`），所以扫描按 id BFS，
//! 比百度按路径更省请求。曲目引用 `/aliyundrive/{file_id}.{ext}`。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::build_blocking_client_for;

const API_BASE: &str = "https://openapi.alipan.com";
/// 曲目引用前缀：`/aliyundrive/{file_id}.{ext}`。
const REF_PREFIX: &str = "/aliyundrive/";
/// 阿里 openFile/list 单页上限 200。
const PAGE_SIZE: usize = 200;
const MAX_PAGES: usize = 20_000;
const MAX_DIRS: usize = 5_000;
const REFRESH_SKEW_SECS: i64 = 120;
const DEFAULT_UA: &str = "Mozilla/5.0 BaYin/1.0";

// ==================== 曲目引用 ====================

/// 合成引用：`/aliyundrive/{file_id}.{ext}`。
pub fn track_path(file_id: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!(
        "{REF_PREFIX}{file_id}.{}",
        if ext.is_empty() { "bin" } else { &ext }
    )
}

/// 从合成引用反解 `file_id`（阿里的 id 是形如 `65a1b2c3...` 的十六进制串）。
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
    refresh_token: String,
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

fn current_access_token(config: &StreamServerConfig, force_refresh: bool) -> Result<String, String> {
    let original = refresh_token_of(config);

    if !force_refresh {
        if let Some(key) = original.as_ref() {
            if let Ok(cache) = token_cache().lock() {
                if let Some(cached) = cache.get(key) {
                    if cached.expires_at > now_secs() + REFRESH_SKEW_SECS {
                        return Ok(cached.access_token.clone());
                    }
                }
            }
        }
    }

    let (client_id, client_secret) = client_credentials(config)
        .ok_or_else(|| "阿里云盘：请填写 client_id 与 client_secret".to_string())?;
    let original = original.ok_or_else(|| "阿里云盘：请填写刷新令牌（refresh_token）".to_string())?;

    // 若历史刷新返回过新 refresh_token，用最新的那个。
    let current_refresh = token_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(&original).map(|t| t.refresh_token.clone()))
        .unwrap_or_else(|| original.clone());

    let response = client()?
        .post(format!("{API_BASE}/oauth/access_token"))
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .json(&json!({
            "grant_type": "refresh_token",
            "refresh_token": current_refresh,
            "client_id": client_id,
            "client_secret": client_secret,
        }))
        .send()
        .map_err(|e| format!("阿里云盘刷新令牌请求失败: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| format!("阿里云盘刷新令牌响应不是 JSON（HTTP {status}）: {e}"))?;

    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            let detail = body
                .get("message")
                .or_else(|| body.get("error_description"))
                .or_else(|| body.get("code"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| body.to_string());
            format!("阿里云盘刷新令牌失败: {detail}")
        })?
        .to_string();
    let refresh_token = body
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .unwrap_or(&current_refresh)
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(7200);

    let token = CachedToken {
        access_token,
        refresh_token,
        expires_at: now_secs() + expires_in,
    };
    if let Ok(mut cache) = token_cache().lock() {
        cache.insert(original, token.clone());
    }
    Ok(token.access_token)
}

// ==================== HTTP ====================

#[derive(Debug)]
enum AliError {
    Unauthorized,
    Failed(String),
}

impl AliError {
    fn message(self) -> String {
        match self {
            AliError::Unauthorized => "阿里云盘：令牌已失效，且刷新失败".to_string(),
            AliError::Failed(message) => message,
        }
    }
}

fn error_message(body: &Value) -> String {
    body.get("message")
        .or_else(|| body.get("code"))
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string())
}

/// 阿里用 HTTP 状态码 + `code` 字段表达错误；401 与 `AccessTokenInvalid` 都算失效。
fn is_unauthorized_body(body: &Value) -> bool {
    body.get("code")
        .and_then(Value::as_str)
        .map(|code| {
            matches!(
                code,
                "AccessTokenInvalid"
                    | "InvalidParameter.RefreshToken"
                    | "TokenExpired"
                    | "AccessTokenExpired"
            )
        })
        .unwrap_or(false)
}

fn parse_response(response: reqwest::blocking::Response) -> Result<Value, AliError> {
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| AliError::Failed(format!("阿里云盘响应不是 JSON（HTTP {status}）: {e}")))?;

    if status.as_u16() == 401 || is_unauthorized_body(&body) {
        return Err(AliError::Unauthorized);
    }
    if !status.is_success() {
        return Err(AliError::Failed(format!(
            "阿里云盘 HTTP {status}: {}",
            error_message(&body)
        )));
    }
    Ok(body)
}

fn post_json(
    client: &reqwest::blocking::Client,
    url: &str,
    payload: &Value,
    access_token: &str,
) -> Result<Value, AliError> {
    let response = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {access_token}"))
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .json(payload)
        .send()
        .map_err(|e| AliError::Failed(format!("阿里云盘请求失败: {e}")))?;
    parse_response(response)
}

fn with_token_retry<T>(
    config: &StreamServerConfig,
    call: impl Fn(&str) -> Result<T, AliError>,
) -> Result<T, String> {
    let token = current_access_token(config, false)?;
    match call(&token) {
        Ok(value) => Ok(value),
        Err(AliError::Unauthorized) => {
            let token = current_access_token(config, true)?;
            call(&token).map_err(AliError::message)
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
    content_hash: Option<String>,
    modified: Option<i64>,
}

fn parse_iso8601(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(raw.trim())
        .ok()
        .map(|dt| dt.timestamp())
}

fn parse_item(value: &Value) -> Option<RemoteItem> {
    let name = value.get("name").and_then(Value::as_str)?.to_string();
    let file_id = value.get("file_id").and_then(Value::as_str)?.to_string();
    if file_id.is_empty() {
        return None;
    }
    // `type` 是 "folder" / "file"。
    let is_dir = value.get("type").and_then(Value::as_str) == Some("folder");

    Some(RemoteItem {
        file_id,
        name,
        size: value.get("size").map(value_to_i64).unwrap_or(0).max(0) as u64,
        is_dir,
        content_hash: value
            .get("content_hash")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        modified: value
            .get("updated_at")
            .and_then(Value::as_str)
            .and_then(parse_iso8601),
    })
}

struct ListPage {
    items: Vec<RemoteItem>,
    next_marker: Option<String>,
}

fn parse_list_page(body: &Value) -> Result<ListPage, AliError> {
    let items = body
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| AliError::Failed("阿里云盘目录响应缺少 items".to_string()))?;
    Ok(ListPage {
        items: items.iter().filter_map(parse_item).collect(),
        next_marker: body
            .get("next_marker")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
            .map(str::to_string),
    })
}

fn parse_drive_id(body: &Value) -> Option<String> {
    body.get("resource_drive_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .or_else(|| {
            body.get("default_drive_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
        })
        .map(str::to_string)
}

// ==================== 列表 / 扫描 ====================

fn root_dir_id(config: &StreamServerConfig) -> String {
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

fn need_drive_id() -> String {
    "阿里云盘：未能获取 drive_id，请检查授权".to_string()
}

/// 取得 drive_id（每次扫描/播放都要用，进程内缓存一次即可）。
fn drive_id(
    client: &reqwest::blocking::Client,
    config: &StreamServerConfig,
) -> Result<String, String> {
    static DRIVE_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let cache = DRIVE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = format!(
        "{}|{}",
        config.access_token.as_deref().unwrap_or(""),
        config.username.trim()
    );
    if let Ok(guard) = cache.lock() {
        if let Some(id) = guard.get(&key) {
            return Ok(id.clone());
        }
    }

    let mut found: Option<String> = None;
    let body = with_token_retry(config, |token| {
        post_json(
            client,
            &format!("{API_BASE}/adrive/v1.0/user/getDriveInfo"),
            &json!({}),
            token,
        )
    })?;
    if let Some(id) = parse_drive_id(&body) {
        found = Some(id);
    } else if let Some(id) = body
        .get("drive_list")
        .and_then(Value::as_array)
        .and_then(|list| {
            list.iter()
                .find_map(|d| d.get("drive_id").and_then(Value::as_str))
        })
    {
        found = Some(id.to_string());
    }

    let id = found.ok_or_else(need_drive_id)?;
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, id.clone());
    }
    Ok(id)
}

fn list_page(
    client: &reqwest::blocking::Client,
    token: &str,
    drive: &str,
    parent_id: &str,
    marker: Option<&str>,
) -> Result<ListPage, AliError> {
    let mut payload = json!({
        "drive_id": drive,
        "parent_file_id": parent_id,
        "limit": PAGE_SIZE,
        "order_by": "name",
        "order_direction": "ASC",
        "fields": "*",
    });
    if let Some(marker) = marker {
        payload["marker"] = json!(marker);
    }
    let body = post_json(
        client,
        &format!("{API_BASE}/adrive/v1.0/openFile/list"),
        &payload,
        token,
    )?;
    parse_list_page(&body)
}

fn list_page_with_retry(
    config: &StreamServerConfig,
    client: &reqwest::blocking::Client,
    drive: &str,
    parent_id: &str,
    marker: Option<&str>,
) -> Result<ListPage, String> {
    with_token_retry(config, |token| {
        list_page(client, token, drive, parent_id, marker)
    })
}

/// 递归扫描曲库（目录 BFS，按 file_id）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    _cover_cache: Option<&crate::utils::cover::CoverCache>,
    _existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    _read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let client = client()?;
    let drive = drive_id(&client, config)?;

    let mut queue: VecDeque<String> = VecDeque::from([root_dir_id(config)]);
    let mut visited_dirs: HashSet<String> = HashSet::new();
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut songs: Vec<ScannedSong> = Vec::new();
    let mut dir_count = 0usize;

    while let Some(parent_id) = queue.pop_front() {
        if !visited_dirs.insert(parent_id.clone()) {
            continue;
        }
        dir_count += 1;
        if dir_count > MAX_DIRS {
            return Err("阿里云盘目录数量超过安全上限，请检查起始目录设置".to_string());
        }

        let mut marker: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let page =
                list_page_with_retry(config, &client, &drive, &parent_id, marker.as_deref())?;

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
                    log::debug!("[阿里云盘] 跳过无可用格式的文件: {} (.{raw_ext})", item.name);
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

                let _ = &item.content_hash; // 预留：增量扫描可用 sha1 作 revision
            }

            if let Some(tx) = &progress {
                let count = songs.len();
                let _ = tx.send((count, count.max(1)));
            }

            match page.next_marker {
                Some(next) => {
                    // 防御：服务端若原样回传同一个 marker，就会死循环。
                    if Some(next.as_str()) == marker.as_deref() {
                        break;
                    }
                    marker = Some(next);
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

pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if client_credentials(config).is_none() || refresh_token_of(config).is_none() {
        return ConnectionTestResult {
            success: false,
            message: "请填写阿里云盘的 client_id、client_secret 与刷新令牌".to_string(),
            server_version: None,
        };
    }
    let client = match client() {
        Ok(client) => client,
        Err(error) => {
            return ConnectionTestResult {
                success: false,
                message: error,
                server_version: None,
            }
        }
    };
    match drive_id(&client, config) {
        Ok(id) => ConnectionTestResult {
            success: true,
            message: format!("已连接到阿里云盘（drive_id: {id}）"),
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

fn resolve_download_url(
    config: &StreamServerConfig,
    file_id: &str,
) -> Result<String, String> {
    let client = client()?;
    let drive = drive_id(&client, config)?;
    let body = with_token_retry(config, |token| {
        post_json(
            &client,
            &format!("{API_BASE}/adrive/v1.0/openFile/getDownloadUrl"),
            &json!({"drive_id": drive, "file_id": file_id}),
            token,
        )
    })?;
    body.get("url")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "阿里云盘下载地址响应缺少 url".to_string())
}

/// 解析直链（HTTP 原生，无需桥接）。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let Some(file_id) = track_file_id(song_id) else {
        log::error!("[阿里云盘] 无法识别的曲目引用: {song_id}");
        return String::new();
    };
    match resolve_download_url(config, &file_id) {
        Ok(url) => url,
        Err(error) => {
            log::error!("[阿里云盘] 换取直链失败（{file_id}）: {error}");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_reference_round_trips() {
        let path = track_path("65a1b2c3d4e5f6", ".FLAC");
        assert_eq!(path, "/aliyundrive/65a1b2c3d4e5f6.flac");
        assert_eq!(
            track_file_id(&path).as_deref(),
            Some("65a1b2c3d4e5f6")
        );
        assert!(track_file_id("/baidupan/123.flac").is_none());
        assert!(track_file_id("/aliyundrive/a/b c.flac").is_none());
    }

    #[test]
    fn parses_folder_and_file_items() {
        let folder = serde_json::json!({
            "file_id": "root-folder", "name": "Music", "type": "folder"
        });
        let file = serde_json::json!({
            "file_id": "65a1", "name": "song.flac", "type": "file",
            "size": 4096, "content_hash": "ABC",
            "updated_at": "2026-10-17T02:00:00.000Z"
        });

        let folder = parse_item(&folder).expect("folder");
        assert!(folder.is_dir);
        assert_eq!(folder.file_id, "root-folder");

        let file = parse_item(&file).expect("file");
        assert!(!file.is_dir);
        assert_eq!(file.size, 4096);
        assert_eq!(file.content_hash.as_deref(), Some("ABC"));
        assert!(file.modified.is_some());
    }

    #[test]
    fn parses_list_page_marker() {
        let body = serde_json::json!({
            "items": [
                {"file_id": "a", "name": "x", "type": "folder"},
                {"file_id": "b", "name": "y.flac", "type": "file", "size": 1}
            ],
            "next_marker": "next-1"
        });
        let page = parse_list_page(&body).expect("page");
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.next_marker.as_deref(), Some("next-1"));

        let last = serde_json::json!({"items": []});
        assert!(parse_list_page(&last).expect("page").next_marker.is_none());
    }

    #[test]
    fn detects_unauthorized_codes() {
        assert!(is_unauthorized_body(
            &serde_json::json!({"code": "AccessTokenInvalid"})
        ));
        assert!(is_unauthorized_body(
            &serde_json::json!({"code": "TokenExpired"})
        ));
        assert!(!is_unauthorized_body(&serde_json::json!({"code": "NotFound"})));
        assert!(!is_unauthorized_body(&serde_json::json!({})));
    }

    #[test]
    fn resolves_drive_id_from_both_shapes() {
        assert_eq!(
            parse_drive_id(&serde_json::json!({"resource_drive_id": "r1"})).as_deref(),
            Some("r1")
        );
        assert_eq!(
            parse_drive_id(&serde_json::json!({"default_drive_id": "d1"})).as_deref(),
            Some("d1")
        );
        assert!(parse_drive_id(&serde_json::json!({})).is_none());
    }

    #[test]
    fn defaults_root_to_root_id() {
        let mut config = StreamServerConfig {
            server_type: crate::models::ServerType::Aliyun,
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
        assert_eq!(root_dir_id(&config), "root");
        config.base_path = Some("/abc/".to_string());
        assert_eq!(root_dir_id(&config), "abc");
    }
}
