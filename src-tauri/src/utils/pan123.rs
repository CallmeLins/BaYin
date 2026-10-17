//! 123 云盘（开放平台 OpenAPI，阶段 D2）。
//!
//! ## 认证模型
//!
//! 123 开放平台有两条路：
//!
//! 1. **OAuth 授权码**（primuse 用的那条）——需要一个已登记的 `https`/自定义
//!    scheme 回调地址，桌面端还得起本地回环或注册 scheme。
//! 2. **开发者授权模式**（本模块用的这条）——直接用 `clientID` + `clientSecret`
//!    换 `access_token`，不需要任何回调。
//!
//! BaYin 走第 2 条：`clientID` 存 `username`，`clientSecret` 存 `password`
//! （走 keyring），`access_token` 留作可选的手工覆盖。
//!
//! ## 协议要点（已对照 OpenListTeam/OpenList `drivers/123_open` 核实）
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 换令牌 | `POST {api}/api/v1/access_token`，头 `Platform: open_platform`，体 `{clientID, clientSecret}` |
//! | 用户信息 | `GET {api}/api/v1/user/info` |
//! | 列目录 | `GET {api}/api/v2/file/list?parentFileId=&limit=&lastFileId=&trashed=false` |
//! | 直链 | `GET {api}/api/v1/file/download_info?fileId=` |
//!
//! 除换令牌外，所有请求带 `Authorization: Bearer <access_token>` 与
//! `Platform: open_platform`。业务错误用 body 里的 `code`（0 成功、401 令牌失效、
//! 429 触发频控）。
//!
//! ## 令牌生命周期
//!
//! 123 **不轮换令牌**，只给 `access_token` + `expiredAt`（通常 30 天）。因此
//! 换到之后进程内缓存即可，401 时重新换一次。换令牌接口有频控，缓存要尽量命中。
//!
//! ⚠️ 跨重启写回尚未实现（`SourceConnector` 拿不到 `server_id`）。好在 123 的
//! `clientID`/`clientSecret` 不过期，重启后重新换一次令牌即可，不需要用户干预。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::build_blocking_client_for;

const API_BASE: &str = "https://open-api.123pan.com";
/// 曲目引用前缀：`/pan123/{file_id}.{ext}`。
const REF_PREFIX: &str = "/pan123/";
/// 开放平台单页上限是 100。
const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 20_000;
const MAX_DIRS: usize = 5_000;
/// 提前多少秒视为过期，避免边界上刚好失效。
const REFRESH_SKEW_SECS: i64 = 300;
/// `expiredAt` 解析失败时的保守缓存时长（123 正常是 30 天）。
const FALLBACK_TTL_SECS: i64 = 6 * 3600;

// ==================== 曲目引用 ====================

/// 合成引用：`/pan123/{file_id}.{ext}`。
pub fn track_path(file_id: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!(
        "{REF_PREFIX}{file_id}.{}",
        if ext.is_empty() { "bin" } else { &ext }
    )
}

/// 从合成引用反解 `file_id`。
pub fn track_file_id(path: &str) -> Option<String> {
    let rest = path.strip_prefix(REF_PREFIX)?;
    let name = rest.rsplit('/').next().unwrap_or(rest);
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => stem,
        _ => name,
    };
    // 123 的 fileId 是纯数字；挡掉明显不是引用的输入。
    (!stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit())).then(|| stem.to_string())
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

fn credentials(config: &StreamServerConfig) -> Option<(String, String)> {
    let client_id = config.username.trim();
    let client_secret = config.password.trim();
    (!client_id.is_empty() && !client_secret.is_empty())
        .then(|| (client_id.to_string(), client_secret.to_string()))
}

fn cache_key(client_id: &str, client_secret: &str) -> String {
    format!("{client_id}\u{1f}{client_secret}")
}

fn client() -> Result<reqwest::blocking::Client, String> {
    build_blocking_client_for(API_BASE)
}

/// 解析 `expiredAt`。123 给的是本地时间字符串，可能是 RFC3339 也可能是
/// `YYYY-MM-DD HH:MM:SS`（无时区，按 UTC+8 解释）。
fn parse_expired_at(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp());
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(raw, format) {
            if let Some(dt) = naive.and_local_timezone(chrono::FixedOffset::east_opt(8 * 3600)?).single() {
                return Some(dt.timestamp());
            }
        }
    }
    None
}

/// 取得可用 access_token。`force_refresh` 用于 401 后强制换新。
fn current_access_token(
    config: &StreamServerConfig,
    force_refresh: bool,
) -> Result<String, String> {
    let creds = credentials(config);

    if !force_refresh {
        // 1) 进程内缓存（clientID+secret 不过期，命中率很高）。
        if let Some((client_id, client_secret)) = creds.as_ref() {
            if let Ok(cache) = token_cache().lock() {
                if let Some(cached) = cache.get(&cache_key(client_id, client_secret)) {
                    if cached.expires_at > now_secs() + REFRESH_SKEW_SECS {
                        return Ok(cached.access_token.clone());
                    }
                }
            }
        }
        // 2) 用户手工粘贴的 access_token。
        if let Some(token) = config
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            return Ok(token.to_string());
        }
    }

    // 3) 用 clientID + clientSecret 换新。
    match creds {
        Some((client_id, client_secret)) => {
            acquire_token(&client_id, &client_secret).map(|token| token.access_token)
        }
        None => Err("123 云盘：请填写 clientID 与 clientSecret".to_string()),
    }
}

fn acquire_token(client_id: &str, client_secret: &str) -> Result<CachedToken, String> {
    let response = client()?
        .post(format!("{API_BASE}/api/v1/access_token"))
        .header("Platform", "open_platform")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({
            "clientID": client_id,
            "clientSecret": client_secret,
        }))
        .send()
        .map_err(|e| format!("123 换令牌请求失败: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| format!("123 换令牌响应不是 JSON（HTTP {status}）: {e}"))?;

    if body.get("code").and_then(Value::as_i64).unwrap_or(-1) != 0 {
        return Err(format!("123 换令牌失败: {}", error_message(&body)));
    }

    let data = body.get("data").cloned().unwrap_or(Value::Null);
    let access_token = data
        .get("accessToken")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "123 换令牌响应缺少 accessToken".to_string())?
        .to_string();

    let expires_at = data
        .get("expiredAt")
        .and_then(Value::as_str)
        .and_then(parse_expired_at)
        .unwrap_or_else(|| now_secs() + FALLBACK_TTL_SECS);

    let token = CachedToken {
        access_token,
        expires_at,
    };
    if let Ok(mut cache) = token_cache().lock() {
        cache.insert(cache_key(client_id, client_secret), token.clone());
    }
    Ok(token)
}

// ==================== HTTP ====================

#[derive(Debug)]
enum PanError {
    Unauthorized,
    RateLimited,
    Failed(String),
}

impl PanError {
    fn message(self) -> String {
        match self {
            PanError::Unauthorized => "123 云盘：令牌已失效，且重新获取失败".to_string(),
            PanError::RateLimited => "123 云盘：请求过于频繁，请稍后重试".to_string(),
            PanError::Failed(message) => message,
        }
    }
}

fn error_message(body: &Value) -> String {
    body.get("message")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string())
}

fn is_unauthorized_body(body: &Value) -> bool {
    body.get("code").and_then(Value::as_i64) == Some(401)
}

fn is_rate_limited_body(body: &Value) -> bool {
    body.get("code").and_then(Value::as_i64) == Some(429)
}

fn parse_response(response: reqwest::blocking::Response) -> Result<Value, PanError> {
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| PanError::Failed(format!("123 响应不是 JSON（HTTP {status}）: {e}")))?;

    if status.as_u16() == 401 || is_unauthorized_body(&body) {
        return Err(PanError::Unauthorized);
    }
    if status.as_u16() == 429 || is_rate_limited_body(&body) {
        return Err(PanError::RateLimited);
    }
    if !status.is_success() {
        return Err(PanError::Failed(format!(
            "123 HTTP {status}: {}",
            error_message(&body)
        )));
    }
    if body.get("code").and_then(Value::as_i64).unwrap_or(0) != 0 {
        return Err(PanError::Failed(format!(
            "123 API 错误: {}",
            error_message(&body)
        )));
    }
    Ok(body)
}

fn get_json(
    client: &reqwest::blocking::Client,
    url: &str,
    token: &str,
) -> Result<Value, PanError> {
    let response = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header("Platform", "open_platform")
        .send()
        .map_err(|e| PanError::Failed(format!("123 请求失败: {e}")))?;
    parse_response(response)
}

/// 执行一次带令牌的调用；401 时强制换令牌并重试一次。
fn with_token_retry<T>(
    config: &StreamServerConfig,
    call: impl Fn(&str) -> Result<T, PanError>,
) -> Result<T, String> {
    let token = current_access_token(config, false)?;
    match call(&token) {
        Ok(value) => Ok(value),
        Err(PanError::Unauthorized) => {
            let token = current_access_token(config, true)?;
            call(&token).map_err(PanError::message)
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

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

/// `updateAt` / `createAt` 形如 `2026-10-17 10:00:00`（UTC+8，无时区标记）。
fn parse_local_datetime(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp());
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(raw, format) {
            if let Some(dt) = naive
                .and_local_timezone(chrono::FixedOffset::east_opt(8 * 3600)?)
                .single()
            {
                return Some(dt.timestamp());
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
struct RemoteItem {
    file_id: String,
    name: String,
    size: u64,
    is_dir: bool,
    etag: Option<String>,
    modified: Option<i64>,
}

fn parse_item(value: &Value) -> Option<RemoteItem> {
    let name = value
        .get("filename")
        .or_else(|| value.get("fileName"))
        .and_then(Value::as_str)?
        .to_string();

    let file_id = value
        .get("fileId")
        .or_else(|| value.get("fileID"))
        .map(value_to_string)
        .filter(|id| !id.is_empty())?;
    // `type`：0 = 文件，1 = 目录。
    let is_dir = value.get("type").map(value_to_i64).unwrap_or(0) == 1;

    Some(RemoteItem {
        file_id,
        name,
        size: value.get("size").map(value_to_i64).unwrap_or(0).max(0) as u64,
        is_dir,
        etag: value
            .get("etag")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        modified: value
            .get("updateAt")
            .and_then(Value::as_str)
            .and_then(parse_local_datetime),
    })
}

struct ListPage {
    items: Vec<RemoteItem>,
    raw_count: usize,
    last_file_id: Option<i64>,
}

fn parse_list_page(body: &Value) -> Result<ListPage, PanError> {
    let data = body
        .get("data")
        .ok_or_else(|| PanError::Failed("123 目录响应缺少 data".to_string()))?;
    let list = data
        .get("fileList")
        .and_then(Value::as_array)
        .ok_or_else(|| PanError::Failed("123 目录响应缺少 fileList".to_string()))?;

    Ok(ListPage {
        raw_count: list.len(),
        items: list.iter().filter_map(parse_item).collect(),
        last_file_id: data.get("lastFileId").and_then(Value::as_i64),
    })
}

fn parse_download_url(body: &Value) -> Option<String> {
    body.get("data")?
        .get("downloadUrl")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ==================== 列表 / 扫描 ====================

fn list_page(
    client: &reqwest::blocking::Client,
    token: &str,
    parent_id: &str,
    last_file_id: i64,
) -> Result<ListPage, PanError> {
    let url = format!(
        "{API_BASE}/api/v2/file/list?parentFileId={parent_id}&limit={PAGE_SIZE}\
         &lastFileId={last_file_id}&trashed=false&searchMode=&searchData="
    );
    let body = get_json(client, &url, token)?;
    parse_list_page(&body)
}

fn list_page_with_retry(
    config: &StreamServerConfig,
    client: &reqwest::blocking::Client,
    parent_id: &str,
    last_file_id: i64,
) -> Result<ListPage, String> {
    with_token_retry(config, |token| {
        list_page(client, token, parent_id, last_file_id)
    })
}

fn root_id(config: &StreamServerConfig) -> String {
    let raw = config
        .base_path
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .trim_start_matches('/');
    if raw.is_empty() {
        // 123 的根目录固定是 0。
        "0".to_string()
    } else {
        raw.to_string()
    }
}

/// 递归扫描曲库（目录 BFS）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    _cover_cache: Option<&crate::utils::cover::CoverCache>,
    _existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    _read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let client = client()?;

    let mut queue: VecDeque<String> = VecDeque::from([root_id(config)]);
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
            return Err("123 云盘目录数量超过安全上限，请检查根目录设置".to_string());
        }

        let mut last_file_id = 0i64;
        for _ in 0..MAX_PAGES {
            let page = list_page_with_retry(config, &client, &parent_id, last_file_id)?;

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
                    log::debug!("[123] 跳过无可用格式的文件: {} (.{raw_ext})", item.name);
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

                let _ = &item.etag; // 预留：增量扫描可用 etag 作 revision
            }

            if let Some(tx) = &progress {
                let count = songs.len();
                let _ = tx.send((count, count.max(1)));
            }

            // 单页没满说明到底了；否则用游标继续。
            let next_cursor = page.last_file_id.unwrap_or(0);
            if page.raw_count < PAGE_SIZE || next_cursor == 0 || next_cursor == last_file_id {
                break;
            }
            last_file_id = next_cursor;
        }
    }

    if let Some(tx) = &progress {
        let total = songs.len();
        let _ = tx.send((total, total));
    }
    Ok(songs)
}

// ==================== 连接测试 ====================

fn fetch_user_info(config: &StreamServerConfig) -> Result<(String, u64, u64), String> {
    let client = client()?;
    let body = with_token_retry(config, |token| {
        get_json(&client, &format!("{API_BASE}/api/v1/user/info"), token)
    })?;
    let data = body.get("data").cloned().unwrap_or(Value::Null);
    let uid = data.get("uid").map(value_to_string).unwrap_or_default();
    let used = data.get("spaceUsed").map(value_to_i64).unwrap_or(0).max(0) as u64;
    let total = data
        .get("spacePermanent")
        .map(value_to_i64)
        .unwrap_or(0)
        .max(0) as u64;
    Ok((uid, used, total))
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if credentials(config).is_none()
        && config.access_token.as_deref().unwrap_or("").trim().is_empty()
    {
        return ConnectionTestResult {
            success: false,
            message: "请填写 123 云盘的 clientID 与 clientSecret".to_string(),
            server_version: None,
        };
    }
    match fetch_user_info(config) {
        Ok((uid, used, total)) => ConnectionTestResult {
            success: true,
            message: if total > 0 {
                format!(
                    "已连接到 123 云盘（UID {uid}，已用 {} / {}）",
                    format_bytes(used),
                    format_bytes(total)
                )
            } else {
                format!("已连接到 123 云盘（UID {uid}）")
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

fn resolve_download_url(config: &StreamServerConfig, file_id: &str) -> Result<String, String> {
    let client = client()?;
    let body = with_token_retry(config, |token| {
        get_json(
            &client,
            &format!("{API_BASE}/api/v1/file/download_info?fileId={file_id}"),
            token,
        )
    })?;
    parse_download_url(&body).ok_or_else(|| "123 直链响应缺少 downloadUrl".to_string())
}

/// 解析直链（HTTP 原生，无需桥接）。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let Some(file_id) = track_file_id(song_id) else {
        log::error!("[123] 无法识别的曲目引用: {song_id}");
        return String::new();
    };
    match resolve_download_url(config, &file_id) {
        Ok(url) => url,
        Err(error) => {
            log::error!("[123] 换取直链失败（{file_id}）: {error}");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_reference_round_trips() {
        let path = track_path("123456", ".FLAC");
        assert_eq!(path, "/pan123/123456.flac");
        assert_eq!(track_file_id(&path).as_deref(), Some("123456"));
        assert_eq!(track_file_id("/pan123/123456").as_deref(), Some("123456"));
        // 别的源的引用不该被认成 123。
        assert!(track_file_id("/pan115/abc123.flac").is_none());
        // 非数字主键（比如误配的 pick_code）也不认。
        assert!(track_file_id("/pan123/abc123.flac").is_none());
    }

    #[test]
    fn parses_folder_and_file_items() {
        let folder = serde_json::json!({
            "fileId": 100, "filename": "Music", "type": 1, "size": 0
        });
        let file = serde_json::json!({
            "fileId": 456, "filename": "song.flac", "type": 0,
            "size": "1024", "etag": "ABC", "updateAt": "2026-10-17 10:00:00"
        });

        let folder = parse_item(&folder).expect("folder");
        assert!(folder.is_dir);
        assert_eq!(folder.file_id, "100");

        let file = parse_item(&file).expect("file");
        assert!(!file.is_dir);
        assert_eq!(file.file_id, "456");
        assert_eq!(file.size, 1024);
        assert_eq!(file.etag.as_deref(), Some("ABC"));
        assert!(file.modified.is_some());
    }

    #[test]
    fn detects_error_codes() {
        assert!(is_unauthorized_body(&serde_json::json!({"code": 401})));
        assert!(is_rate_limited_body(&serde_json::json!({"code": 429})));
        assert!(!is_unauthorized_body(&serde_json::json!({"code": 0})));
        assert!(!is_rate_limited_body(&serde_json::json!({"code": 0})));
    }

    #[test]
    fn parses_file_list_page() {
        let body = serde_json::json!({
            "code": 0,
            "data": {
                "lastFileId": 456,
                "fileList": [
                    {"fileId": 1, "filename": "a", "type": 1, "size": 0},
                    {"fileId": 2, "filename": "b.flac", "type": 0, "size": 10}
                ]
            }
        });
        let page = parse_list_page(&body).expect("page");
        assert_eq!(page.raw_count, 2);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.last_file_id, Some(456));
    }

    #[test]
    fn parses_download_url() {
        let body = serde_json::json!({
            "code": 0,
            "data": {"downloadUrl": "https://cdn/1.flac"}
        });
        assert_eq!(
            parse_download_url(&body).as_deref(),
            Some("https://cdn/1.flac")
        );
        assert!(parse_download_url(&serde_json::json!({"code": 0, "data": {}})).is_none());
    }

    #[test]
    fn parses_local_expiry_timestamp() {
        // `expiredAt` 没有时区标记，按 UTC+8 解释。
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 10, 17)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(parse_expired_at("2026-10-17 08:00:00"), Some(expected));
        assert!(parse_expired_at("").is_none());
    }
}
