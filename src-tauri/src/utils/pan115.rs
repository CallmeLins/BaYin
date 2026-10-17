//! 115 网盘（开放平台 OpenAPI，阶段 D1）。
//!
//! ## 认证模型
//!
//! 115 开放平台不使用浏览器重定向 OAuth，而是「应用 AppID + 设备码/扫码」换令牌。
//! 因此 BaYin 只需要用户填入一对令牌：
//!
//! - `access_token` → 配置里的 `access_token`
//! - `refresh_token` → 配置里的 `password`（凭据列，走 keyring）
//!
//! 令牌可在 115 开放平台的应用页或 OpenList 的 `api.oplist.org` 生成。
//!
//! ## 协议要点（已对照 OpenListTeam/115-sdk-go 源码核实）
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 用户信息 | `GET {api}/open/user/info` |
//! | 列目录 | `GET {api}/open/ufile/files?cid=&limit=&offset=&show_dir=1` |
//! | 直链 | `POST {api}/open/ufile/downurl`，form `pick_code` |
//! | 刷新 | `POST {passport}/open/refreshToken`，form `refresh_token` |
//!
//! 认证头统一为 `Authorization: Bearer <access_token>`。
//!
//! ## 令牌轮换
//!
//! **刷新会轮换 `refresh_token`**（SDK 的 `RefreshToken` 会同时写回 access 与
//! refresh，并触发 `onRefreshToken` 回调）。本模块把新令牌写进进程内缓存，
//! key 用用户最初填入的 refresh_token，因此同一会话内不会失效。
//!
//! ⚠️ 跨重启写回 `stream_servers` 尚未实现：`SourceConnector` 目前拿不到
//! `server_id`，无法安全定位要更新的行。若应用重启后旧 refresh_token 已被
//! 轮换，需要用户重新粘贴一对令牌。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::build_blocking_client_for;

const API_BASE: &str = "https://proapi.115.com";
const PASSPORT_BASE: &str = "https://passportapi.115.com";
/// 曲目引用前缀：`/pan115/{pick_code}.{ext}`。
const REF_PREFIX: &str = "/pan115/";
const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 10_000;
const MAX_DIRS: usize = 2_000;
/// 提前多少秒视为过期，避免边界上刚好失效。
const REFRESH_SKEW_SECS: i64 = 120;
/// 115 直链对 UA 敏感，换链与下载必须保持一致。
const DEFAULT_UA: &str = "Mozilla/5.0 BaYin/1.0";

// ==================== 曲目引用 ====================

/// 合成引用：`/pan115/{pick_code}.{ext}`。
pub fn track_path(pick_code: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!(
        "{REF_PREFIX}{pick_code}.{}",
        if ext.is_empty() { "bin" } else { &ext }
    )
}

/// 从合成引用反解 `pick_code`。
pub fn track_pick_code(path: &str) -> Option<String> {
    let rest = path.strip_prefix(REF_PREFIX)?;
    let name = rest.rsplit('/').next().unwrap_or(rest);
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => stem,
        _ => name,
    };
    (!stem.is_empty()).then(|| stem.to_string())
}

// ==================== 令牌 ====================

#[derive(Debug, Clone)]
struct CachedTokens {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

fn token_cache() -> &'static Mutex<HashMap<String, CachedTokens>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedTokens>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn refresh_token_of(config: &StreamServerConfig) -> Option<String> {
    let rt = config.password.trim();
    (!rt.is_empty()).then(|| rt.to_string())
}

fn client() -> Result<reqwest::blocking::Client, String> {
    build_blocking_client_for(API_BASE)
}

/// 取得可用 access_token。`force_refresh` 用于 401 后强制换新。
fn current_access_token(config: &StreamServerConfig, force_refresh: bool) -> Result<String, String> {
    let original = refresh_token_of(config);

    if !force_refresh {
        // 1) 进程内缓存（refresh_token 轮换后的最新值）。
        if let Some(key) = original.as_ref() {
            if let Ok(cache) = token_cache().lock() {
                if let Some(cached) = cache.get(key) {
                    if cached.expires_at > now_secs() + REFRESH_SKEW_SECS {
                        return Ok(cached.access_token.clone());
                    }
                }
            }
        }
        // 2) 用户直接填入的 access_token。
        if let Some(token) = config
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            return Ok(token.to_string());
        }
    }

    // 3) 用 refresh_token 换新（会轮换）。
    if original.is_some() {
        return Ok(refresh_tokens(config)?.access_token);
    }

    Err("115：未填写访问令牌或刷新令牌".to_string())
}

fn refresh_tokens(config: &StreamServerConfig) -> Result<CachedTokens, String> {
    let original = refresh_token_of(config).ok_or("115：未填写刷新令牌（refresh_token）")?;

    // 轮换后的 refresh_token 可能已不等于配置里的原值，优先用缓存里最新的。
    let current_rt = token_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(&original).map(|t| t.refresh_token.clone()))
        .unwrap_or_else(|| original.clone());

    let response = client()?
        .post(format!("{PASSPORT_BASE}/open/refreshToken"))
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .form(&[("refresh_token", current_rt.as_str())])
        .send()
        .map_err(|e| format!("115 刷新令牌请求失败: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| format!("115 刷新令牌响应不是 JSON（HTTP {status}）: {e}"))?;

    let payload = body
        .get("data")
        .filter(|d| d.is_object())
        .cloned()
        .unwrap_or_else(|| body.clone());

    let access_token = payload
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("115 刷新令牌失败: {}", error_message(&body)))?;
    let refresh_token = payload
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or(&current_rt)
        .to_string();
    let expires_in = payload
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(7200);

    let tokens = CachedTokens {
        access_token: access_token.to_string(),
        refresh_token,
        expires_at: now_secs() + expires_in,
    };
    if let Ok(mut cache) = token_cache().lock() {
        cache.insert(original, tokens.clone());
    }
    Ok(tokens)
}

// ==================== HTTP ====================

#[derive(Debug)]
enum PanError {
    Unauthorized,
    Failed(String),
}

impl PanError {
    fn message(self) -> String {
        match self {
            PanError::Unauthorized => "115：令牌已失效，且刷新失败".to_string(),
            PanError::Failed(message) => message,
        }
    }
}

fn error_message(body: &Value) -> String {
    body.get("message")
        .and_then(Value::as_str)
        .or_else(|| body.get("error").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string())
}

fn is_unauthorized_body(body: &Value) -> bool {
    let code = body.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code == 401 || code == 40140116 {
        return true;
    }
    body.get("message")
        .and_then(Value::as_str)
        .map(|m| m.contains("no auth") || m.contains("未登录"))
        .unwrap_or(false)
}

fn parse_response(response: reqwest::blocking::Response) -> Result<Value, PanError> {
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| PanError::Failed(format!("115 响应不是 JSON（HTTP {status}）: {e}")))?;

    if status.as_u16() == 401 || is_unauthorized_body(&body) {
        return Err(PanError::Unauthorized);
    }
    if !status.is_success() {
        return Err(PanError::Failed(format!(
            "115 HTTP {status}: {}",
            error_message(&body)
        )));
    }
    if body.get("state").and_then(Value::as_bool) == Some(false) {
        return Err(PanError::Failed(format!(
            "115 API 错误: {}",
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
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .send()
        .map_err(|e| PanError::Failed(format!("115 请求失败: {e}")))?;
    parse_response(response)
}

fn post_form(
    client: &reqwest::blocking::Client,
    url: &str,
    form: &[(&str, &str)],
    token: &str,
) -> Result<Value, PanError> {
    let response = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .header(reqwest::header::USER_AGENT, DEFAULT_UA)
        .form(form)
        .send()
        .map_err(|e| PanError::Failed(format!("115 请求失败: {e}")))?;
    parse_response(response)
}

/// 执行一次带令牌的调用；401 时强制刷新并重试一次。
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

#[derive(Debug, Clone, PartialEq)]
struct RemoteItem {
    file_id: Option<String>,
    pick_code: String,
    name: String,
    size: u64,
    is_dir: bool,
    sha1: Option<String>,
    modified: Option<i64>,
}

fn parse_item(value: &Value) -> Option<RemoteItem> {
    let name = value
        .get("fn")
        .or_else(|| value.get("n"))
        .and_then(Value::as_str)?
        .to_string();

    let pick_code = value
        .get("pc")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let file_id = value
        .get("fid")
        .or_else(|| value.get("cid"))
        .map(value_to_string)
        .filter(|id| !id.is_empty());

    // `fc` 在不同版本里可能是 "0"/"1" 或数字；缺失时用 pick_code 兜底判断。
    let is_dir = match value.get("fc") {
        Some(Value::String(category)) => category == "0",
        Some(Value::Number(number)) => number.as_i64() == Some(0),
        _ => pick_code.is_empty(),
    };

    if is_dir && file_id.is_none() {
        return None;
    }
    if !is_dir && pick_code.is_empty() {
        return None;
    }

    Some(RemoteItem {
        file_id,
        pick_code,
        name,
        size: value.get("fs").map(value_to_i64).unwrap_or(0).max(0) as u64,
        is_dir,
        sha1: value.get("sha1").and_then(Value::as_str).map(str::to_string),
        modified: value
            .get("upt")
            .or_else(|| value.get("te"))
            .and_then(Value::as_i64),
    })
}

fn parse_list_page(body: &Value) -> Result<(Vec<RemoteItem>, usize), PanError> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| PanError::Failed("115 目录响应缺少 data".to_string()))?;
    let raw = data.len();
    Ok((data.iter().filter_map(parse_item).collect(), raw))
}

/// 从 `downurl` 响应里取直链。`data` 可能以 file_id 为键，也可能直接是对象。
fn parse_downurl(body: &Value) -> Option<String> {
    let data = body.get("data")?;
    // 形态一：data 本身就是 `{ url: "..." }` 或 `{ url: { url: "..." } }`。
    if let Some(url) = url_from_entry(data) {
        return Some(url);
    }
    // 形态二：data 以 file_id 为键，取第一个条目的 url。
    data.as_object()?.values().find_map(url_from_entry)
}

fn url_from_entry(entry: &Value) -> Option<String> {
    let url = entry.get("url")?;
    url.as_str()
        .map(str::to_string)
        .or_else(|| url.get("url").and_then(Value::as_str).map(str::to_string))
}

// ==================== 列表 / 扫描 ====================

fn list_page(
    client: &reqwest::blocking::Client,
    token: &str,
    cid: &str,
    offset: usize,
) -> Result<(Vec<RemoteItem>, usize), PanError> {
    let url = format!(
        "{API_BASE}/open/ufile/files?cid={cid}&limit={PAGE_SIZE}&offset={offset}&show_dir=1"
    );
    let body = get_json(client, &url, token)?;
    parse_list_page(&body)
}

fn list_page_with_retry(
    config: &StreamServerConfig,
    client: &reqwest::blocking::Client,
    cid: &str,
    offset: usize,
) -> Result<(Vec<RemoteItem>, usize), String> {
    with_token_retry(config, |token| list_page(client, token, cid, offset))
}

fn root_cid(config: &StreamServerConfig) -> String {
    let raw = config
        .base_path
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .trim_start_matches('/');
    if raw.is_empty() {
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

    let mut queue: VecDeque<String> = VecDeque::from([root_cid(config)]);
    let mut visited_dirs: HashSet<String> = HashSet::new();
    let mut seen_pick_codes: HashSet<String> = HashSet::new();
    let mut songs: Vec<ScannedSong> = Vec::new();
    let mut dir_count = 0usize;

    while let Some(cid) = queue.pop_front() {
        if !visited_dirs.insert(cid.clone()) {
            continue;
        }
        dir_count += 1;
        if dir_count > MAX_DIRS {
            return Err("115 目录数量超过安全上限，请检查根目录设置".to_string());
        }

        let mut offset = 0usize;
        for _ in 0..MAX_PAGES {
            let (items, raw_count) = list_page_with_retry(config, &client, &cid, offset)?;

            for item in items {
                if item.is_dir {
                    if let Some(dir_id) = item.file_id {
                        queue.push_back(dir_id);
                    }
                    continue;
                }

                let path = Path::new(&item.name);
                let Some(raw_ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                let Some(ext) = normalize_audio_extension(raw_ext) else {
                    log::debug!("[115] 跳过无可用格式的文件: {} (.{raw_ext})", item.name);
                    continue;
                };
                if !seen_pick_codes.insert(item.pick_code.clone()) {
                    continue;
                }

                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&item.name)
                    .to_string();
                let reference = track_path(&item.pick_code, &ext);

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

                let _ = item.sha1; // 预留：增量扫描可用 sha1 作 revision
            }

            if let Some(tx) = &progress {
                let count = songs.len();
                let _ = tx.send((count, count.max(1)));
            }

            if raw_count < PAGE_SIZE {
                break;
            }
            offset += raw_count;
        }
    }

    if let Some(tx) = &progress {
        let total = songs.len();
        let _ = tx.send((total, total));
    }
    Ok(songs)
}

// ==================== 连接测试 ====================

fn fetch_user_info(config: &StreamServerConfig) -> Result<(String, String), String> {
    let client = client()?;
    let body = with_token_retry(config, |token| {
        get_json(&client, &format!("{API_BASE}/open/user/info"), token)
    })?;
    let data = body.get("data").cloned().unwrap_or(Value::Null);
    let id = data.get("user_id").map(value_to_string).unwrap_or_default();
    let name = data
        .get("user_name")
        .and_then(Value::as_str)
        .unwrap_or("115 用户")
        .to_string();
    Ok((id, name))
}

pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if refresh_token_of(config).is_none() && config.access_token.as_deref().unwrap_or("").trim().is_empty() {
        return ConnectionTestResult {
            success: false,
            message: "请填写 115 的访问令牌或刷新令牌".to_string(),
            server_version: None,
        };
    }
    match fetch_user_info(config) {
        Ok((user_id, name)) => ConnectionTestResult {
            success: true,
            message: if user_id.is_empty() {
                format!("已连接到 115 网盘（{name}）")
            } else {
                format!("已连接到 115 网盘（{name} / {user_id}）")
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

fn resolve_download_url(config: &StreamServerConfig, pick_code: &str) -> Result<String, String> {
    let client = client()?;
    let body = with_token_retry(config, |token| {
        post_form(
            &client,
            &format!("{API_BASE}/open/ufile/downurl"),
            &[("pick_code", pick_code)],
            token,
        )
    })?;
    parse_downurl(&body).ok_or_else(|| "115 直链响应缺少 url".to_string())
}

/// 解析直链（HTTP 原生，无需桥接）。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let Some(pick_code) = track_pick_code(song_id) else {
        log::error!("[115] 无法识别的曲目引用: {song_id}");
        return String::new();
    };
    match resolve_download_url(config, &pick_code) {
        Ok(url) => url,
        Err(error) => {
            log::error!("[115] 换取直链失败（{pick_code}）: {error}");
            String::new()
        }
    }
}

/// 直链下载需要与原换链请求一致的 UA。
pub fn stream_headers() -> Vec<(String, String)> {
    vec![("User-Agent".to_string(), DEFAULT_UA.to_string())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_reference_round_trips() {
        let path = track_path("abc123", ".FLAC");
        assert_eq!(path, "/pan115/abc123.flac");
        assert_eq!(track_pick_code(&path).as_deref(), Some("abc123"));
        assert_eq!(track_pick_code("/pan115/abc123").as_deref(), Some("abc123"));
        assert!(track_pick_code("/daoliyu/tracks/1.flac").is_none());
    }

    #[test]
    fn parses_folder_and_file_items() {
        let folder = serde_json::json!({
            "fid": "123", "fn": "Music", "fc": "0", "fs": 0
        });
        let file = serde_json::json!({
            "fid": "456", "pc": "pick-1", "fn": "song.flac", "fc": "1", "fs": "1024", "sha1": "ABC"
        });

        let folder = parse_item(&folder).expect("folder");
        assert!(folder.is_dir);
        assert_eq!(folder.file_id.as_deref(), Some("123"));

        let file = parse_item(&file).expect("file");
        assert!(!file.is_dir);
        assert_eq!(file.pick_code, "pick-1");
        assert_eq!(file.size, 1024);
        assert_eq!(file.sha1.as_deref(), Some("ABC"));
    }

    #[test]
    fn detects_unauthorized_responses() {
        assert!(is_unauthorized_body(
            &serde_json::json!({"code": 401, "message": "no auth"})
        ));
        assert!(is_unauthorized_body(
            &serde_json::json!({"code": 40140116})
        ));
        assert!(!is_unauthorized_body(&serde_json::json!({"code": 0})));
    }

    #[test]
    fn parses_downurl_shapes() {
        let keyed = serde_json::json!({
            "state": true,
            "data": {"456": {"url": {"url": "https://cdn/1.flac"}}}
        });
        assert_eq!(parse_downurl(&keyed).as_deref(), Some("https://cdn/1.flac"));

        let flat = serde_json::json!({
            "state": true,
            "data": {"url": "https://cdn/2.flac"}
        });
        assert_eq!(parse_downurl(&flat).as_deref(), Some("https://cdn/2.flac"));
    }
}
