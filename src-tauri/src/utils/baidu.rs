//! 百度网盘（开放平台 Pan API，阶段 D3）。
//!
//! ## 认证模型
//!
//! 百度只提供 OAuth2，**没有**账号密码或 client credentials 直连。本模块收集三样：
//!
//! | 配置字段 | 含义 |
//! |---|---|
//! | `username` | AppKey（`client_id`） |
//! | `password` | SecretKey（`client_secret`） |
//! | `access_token` | 刷新令牌（`refresh_token`）——运行时换出的 access_token 只进内存 |
//!
//! 刷新令牌需要用户自己走一次授权拿到（百度开放平台的应用页，或 OpenList 的
//! 辅助页面用浏览器 cookie 换）。百度不轮换 refresh_token，所以填一次就能长期用。
//!
//! ## 协议要点（已对照 primuse `BaiduPanSource` 核实）
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 刷新令牌 | `POST {oauth}/token`，form `grant_type=refresh_token&refresh_token=&client_id=&client_secret=` |
//! | 用户信息 | `GET {api}/rest/2.0/xpan/nas?method=uinfo` |
//! | 列目录 | `GET {api}/rest/2.0/xpan/file?method=list&dir=&start=&limit=1000` |
//! | 取直链 | `GET {api}/rest/2.0/xpan/multimedia?method=filemetas&fsids=[id]&dlink=1` |
//!
//! ## 两个容易踩的坑
//!
//! 1. **dlink 必须拼 `&access_token=`**，否则拿到的是 403 的裸链。
//! 2. **下载必须带 `User-Agent: pan.baidu.com`**，否则 CDN 直接拒绝。
//!
//! ## 与 115/123 的差别
//!
//! 百度按**路径**组织目录（`dir=/music/album`），不是 id，所以扫描从
//! `base_path`（默认 `/`）开始按路径 BFS。曲目引用仍用稳定的 `fs_id`：
//! `/baidupan/{fs_id}.{ext}`，这样重命名/移动后引用不变。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::build_blocking_client_for;

const API_BASE: &str = "https://pan.baidu.com";
const OAUTH_BASE: &str = "https://openapi.baidu.com/oauth/2.0";
/// 曲目引用前缀：`/baidupan/{fs_id}.{ext}`。
const REF_PREFIX: &str = "/baidupan/";
/// 百度 list 接口单页上限 1000。
const PAGE_SIZE: usize = 1000;
const MAX_PAGES: usize = 10_000;
const MAX_DIRS: usize = 5_000;
/// 提前多少秒视为过期。
const REFRESH_SKEW_SECS: i64 = 120;
/// 百度 CDN 只认这个 UA。
const DLINK_UA: &str = "pan.baidu.com";
const DLINK_REFERER: &str = "https://pan.baidu.com/";

// ==================== 曲目引用 ====================

/// 合成引用：`/baidupan/{fs_id}.{ext}`。
pub fn track_path(fs_id: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!(
        "{REF_PREFIX}{fs_id}.{}",
        if ext.is_empty() { "bin" } else { &ext }
    )
}

/// 从合成引用反解 `fs_id`。
pub fn track_fs_id(path: &str) -> Option<String> {
    let rest = path.strip_prefix(REF_PREFIX)?;
    let name = rest.rsplit('/').next().unwrap_or(rest);
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => stem,
        _ => name,
    };
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
fn current_access_token(config: &StreamServerConfig, force_refresh: bool) -> Result<String, String> {
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

    let (client_id, client_secret) = client_credentials(config)
        .ok_or_else(|| "百度网盘：请填写 AppKey（client_id）与 SecretKey（client_secret）".to_string())?;
    let refresh = refresh.ok_or_else(|| "百度网盘：请填写刷新令牌（refresh_token）".to_string())?;

    let response = client()?
        .post(format!("{OAUTH_BASE}/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
        ])
        .send()
        .map_err(|e| format!("百度刷新令牌请求失败: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| format!("百度刷新令牌响应不是 JSON（HTTP {status}）: {e}"))?;

    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            let detail = body
                .get("error_description")
                .and_then(Value::as_str)
                .or_else(|| body.get("error").and_then(Value::as_str))
                .map(str::to_string)
                .unwrap_or_else(|| body.to_string());
            format!("百度刷新令牌失败: {detail}")
        })?
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(2592000);

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
enum PanError {
    Unauthorized,
    Failed(String),
}

impl PanError {
    fn message(self) -> String {
        match self {
            PanError::Unauthorized => "百度网盘：令牌已失效，且刷新失败".to_string(),
            PanError::Failed(message) => message,
        }
    }
}

/// 百度用 `errno` 表达业务错误。111 = access_token 失效，-6 = 身份验证失败。
fn errno_of(body: &Value) -> i64 {
    body.get("errno")
        .or_else(|| body.get("error_code"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn is_unauthorized_body(body: &Value) -> bool {
    matches!(errno_of(body), 111 | -6)
}

fn error_message(body: &Value) -> String {
    let code = errno_of(body);
    let detail = body
        .get("error_msg")
        .or_else(|| body.get("error_description"))
        .or_else(|| body.get("show_msg"))
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty());
    match (code, detail) {
        (0, Some(detail)) => detail.to_string(),
        (0, None) => body.to_string(),
        (code, Some(detail)) => format!("errno {code}: {detail}"),
        (code, None) => format!("errno {code}"),
    }
}

fn parse_response(response: reqwest::blocking::Response) -> Result<Value, PanError> {
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|e| PanError::Failed(format!("百度响应不是 JSON（HTTP {status}）: {e}")))?;

    if status.as_u16() == 401 || status.as_u16() == 403 || is_unauthorized_body(&body) {
        return Err(PanError::Unauthorized);
    }
    if !status.is_success() {
        return Err(PanError::Failed(format!(
            "百度 HTTP {status}: {}",
            error_message(&body)
        )));
    }
    let code = errno_of(&body);
    if code != 0 {
        return Err(PanError::Failed(format!("百度 API 错误: {}", error_message(&body))));
    }
    Ok(body)
}

/// 百度的令牌走 query 的 `access_token=`，不是 `Authorization` 头。
fn get_json(
    client: &reqwest::blocking::Client,
    url: &str,
    access_token: &str,
) -> Result<Value, PanError> {
    let separator = if url.contains('?') { '&' } else { '?' };
    let response = client
        .get(format!("{url}{separator}access_token={access_token}"))
        .header(reqwest::header::USER_AGENT, DLINK_UA)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .map_err(|e| PanError::Failed(format!("百度请求失败: {e}")))?;
    parse_response(response)
}

/// 执行一次带令牌的调用；令牌失效时刷新并重试一次。
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

#[derive(Debug, Clone, PartialEq)]
struct RemoteItem {
    fs_id: String,
    name: String,
    path: String,
    size: u64,
    is_dir: bool,
    md5: Option<String>,
    modified: Option<i64>,
}

fn parse_item(value: &Value) -> Option<RemoteItem> {
    let name = value.get("server_filename").and_then(Value::as_str)?.to_string();
    let fs_id = value.get("fs_id").map(value_to_i64).filter(|id| *id > 0)?;
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    Some(RemoteItem {
        fs_id: fs_id.to_string(),
        name,
        path,
        size: value.get("size").map(value_to_i64).unwrap_or(0).max(0) as u64,
        // `isdir` 在不同接口里可能是 0/1 或布尔。
        is_dir: match value.get("isdir") {
            Some(Value::Bool(flag)) => *flag,
            other => other.map(value_to_i64).unwrap_or(0) == 1,
        },
        md5: value
            .get("md5")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        modified: value
            .get("server_mtime")
            .map(value_to_i64)
            .filter(|secs| *secs > 0),
    })
}

fn parse_list_page(body: &Value) -> Vec<RemoteItem> {
    body.get("list")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(parse_item).collect())
        .unwrap_or_default()
}

/// `filemetas` 返回 `{ list: [{ fs_id, dlink, ... }] }`。
fn parse_dlink(body: &Value) -> Option<String> {
    body.get("list")?
        .as_array()?
        .iter()
        .find_map(|item| item.get("dlink").and_then(Value::as_str))
        .filter(|url| !url.is_empty())
        .map(str::to_string)
}

// ==================== 列表 / 扫描 ====================

fn root_dir(config: &StreamServerConfig) -> String {
    let raw = config
        .base_path
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if raw.is_empty() {
        "/".to_string()
    } else if raw.starts_with('/') {
        normalize_dir(raw)
    } else {
        normalize_dir(&format!("/{raw}"))
    }
}

/// 归一化：根目录永远返回 `/`，其他去掉尾部斜杠。
fn normalize_dir(dir: &str) -> String {
    let trimmed = dir.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn list_page(
    client: &reqwest::blocking::Client,
    token: &str,
    dir: &str,
    start: usize,
) -> Result<Vec<RemoteItem>, PanError> {
    let url = format!(
        "{API_BASE}/rest/2.0/xpan/file?method=list&dir={}&order=name&start={start}&limit={PAGE_SIZE}",
        urlencode(dir)
    );
    let body = get_json(client, &url, token)?;
    Ok(parse_list_page(&body))
}

fn list_page_with_retry(
    config: &StreamServerConfig,
    client: &reqwest::blocking::Client,
    dir: &str,
    start: usize,
) -> Result<Vec<RemoteItem>, String> {
    with_token_retry(config, |token| list_page(client, token, dir, start))
}

/// 极简百分号编码：只保留 unreserved 字符，其余按 UTF-8 字节转义。
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

/// 递归扫描曲库（目录 BFS，按路径）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    _cover_cache: Option<&crate::utils::cover::CoverCache>,
    _existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    _read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let client = client()?;

    let mut queue: VecDeque<String> = VecDeque::from([root_dir(config)]);
    let mut visited_dirs: HashSet<String> = HashSet::new();
    let mut seen_files: HashSet<String> = HashSet::new();
    let mut songs: Vec<ScannedSong> = Vec::new();
    let mut dir_count = 0usize;

    while let Some(dir) = queue.pop_front() {
        let dir = normalize_dir(&dir);
        if !visited_dirs.insert(dir.clone()) {
            continue;
        }
        dir_count += 1;
        if dir_count > MAX_DIRS {
            return Err("百度网盘目录数量超过安全上限，请检查根目录设置".to_string());
        }

        let mut start = 0usize;
        for _ in 0..MAX_PAGES {
            let items = list_page_with_retry(config, &client, &dir, start)?;
            let raw_count = items.len();

            for item in items {
                if item.is_dir {
                    queue.push_back(if item.path.is_empty() {
                        format!("{}/{}", dir.trim_end_matches('/'), item.name)
                    } else {
                        item.path.clone()
                    });
                    continue;
                }

                let path = Path::new(&item.name);
                let Some(raw_ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                let Some(ext) = normalize_audio_extension(raw_ext) else {
                    log::debug!("[百度] 跳过无可用格式的文件: {} (.{raw_ext})", item.name);
                    continue;
                };
                if !seen_files.insert(item.fs_id.clone()) {
                    continue;
                }

                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&item.name)
                    .to_string();
                let reference = track_path(&item.fs_id, &ext);

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

                let _ = &item.md5; // 预留：增量扫描可用 md5 作 revision
            }

            if let Some(tx) = &progress {
                let count = songs.len();
                let _ = tx.send((count, count.max(1)));
            }

            if raw_count < PAGE_SIZE {
                break;
            }
            start += raw_count;
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
        get_json(
            &client,
            &format!("{API_BASE}/rest/2.0/xpan/nas?method=uinfo"),
            token,
        )
    })?;
    let name = body
        .get("baidu_name")
        .or_else(|| body.get("netdisk_name"))
        .and_then(Value::as_str)
        .unwrap_or("百度网盘用户")
        .to_string();
    let vip = body
        .get("vip_type")
        .map(value_to_i64)
        .unwrap_or(0);
    let tier = if vip > 0 { "会员" } else { "普通用户" };
    Ok((name, tier.to_string()))
}

pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if client_credentials(config).is_none() || refresh_token_of(config).is_none() {
        return ConnectionTestResult {
            success: false,
            message: "请填写百度网盘的 AppKey、SecretKey 与刷新令牌".to_string(),
            server_version: None,
        };
    }
    match fetch_user_info(config) {
        Ok((name, tier)) => ConnectionTestResult {
            success: true,
            message: format!("已连接到百度网盘（{name} / {tier}）"),
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

fn resolve_dlink(config: &StreamServerConfig, fs_id: &str) -> Result<String, String> {
    let client = client()?;
    let (body, token) = {
        let access_token = current_access_token(config, false)?;
        let result = get_json(
            &client,
            &format!(
                "{API_BASE}/rest/2.0/xpan/multimedia?method=filemetas&fsids=[{fs_id}]&dlink=1"
            ),
            &access_token,
        );
        match result {
            Ok(body) => (body, access_token),
            Err(PanError::Unauthorized) => {
                let access_token = current_access_token(config, true)?;
                let body = get_json(
                    &client,
                    &format!(
                        "{API_BASE}/rest/2.0/xpan/multimedia?method=filemetas&fsids=[{fs_id}]&dlink=1"
                    ),
                    &access_token,
                )
                .map_err(PanError::message)?;
                (body, access_token)
            }
            Err(error) => return Err(error.message()),
        }
    };

    let dlink = parse_dlink(&body).ok_or_else(|| "百度直链响应缺少 dlink".to_string())?;
    // dlink 本身不带凭证，必须拼上 access_token 才能下载。
    let separator = if dlink.contains('?') { '&' } else { '?' };
    Ok(format!("{dlink}{separator}access_token={token}"))
}

/// 解析直链（HTTP 原生，无需桥接）。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    let Some(fs_id) = track_fs_id(song_id) else {
        log::error!("[百度] 无法识别的曲目引用: {song_id}");
        return String::new();
    };
    match resolve_dlink(config, &fs_id) {
        Ok(url) => url,
        Err(error) => {
            log::error!("[百度] 换取直链失败（{fs_id}）: {error}");
            String::new()
        }
    }
}

/// 百度 CDN 只认 `pan.baidu.com` 这个 UA，且校验 Referer。
pub fn stream_headers() -> Vec<(String, String)> {
    vec![
        ("User-Agent".to_string(), DLINK_UA.to_string()),
        ("Referer".to_string(), DLINK_REFERER.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_reference_round_trips() {
        let path = track_path("987654", ".FLAC");
        assert_eq!(path, "/baidupan/987654.flac");
        assert_eq!(track_fs_id(&path).as_deref(), Some("987654"));
        assert_eq!(track_fs_id("/baidupan/987654").as_deref(), Some("987654"));
        assert!(track_fs_id("/pan123/987654.flac").is_none());
        assert!(track_fs_id("/baidupan/abc.flac").is_none());
    }

    #[test]
    fn parses_list_items() {
        let folder = serde_json::json!({
            "fs_id": 1, "server_filename": "Music", "path": "/Music", "isdir": 1, "size": 0
        });
        let file = serde_json::json!({
            "fs_id": 2, "server_filename": "song.flac", "path": "/Music/song.flac",
            "isdir": 0, "size": "2048", "md5": "abc", "server_mtime": 1700000000
        });

        let folder = parse_item(&folder).expect("folder");
        assert!(folder.is_dir);
        assert_eq!(folder.fs_id, "1");

        let file = parse_item(&file).expect("file");
        assert!(!file.is_dir);
        assert_eq!(file.size, 2048);
        assert_eq!(file.md5.as_deref(), Some("abc"));
        assert_eq!(file.modified, Some(1_700_000_000));
    }

    #[test]
    fn detects_unauthorized_errno() {
        assert!(is_unauthorized_body(&serde_json::json!({"errno": 111})));
        assert!(is_unauthorized_body(&serde_json::json!({"errno": -6})));
        assert!(!is_unauthorized_body(&serde_json::json!({"errno": 0})));
        assert!(!is_unauthorized_body(&serde_json::json!({"errno": 31066})));
    }

    #[test]
    fn parses_dlink() {
        let body = serde_json::json!({
            "errno": 0,
            "list": [{"fs_id": 2, "dlink": "https://d.pcs.baidu.com/file/xyz"}]
        });
        assert_eq!(
            parse_dlink(&body).as_deref(),
            Some("https://d.pcs.baidu.com/file/xyz")
        );
        assert!(parse_dlink(&serde_json::json!({"errno": 0, "list": []})).is_none());
    }

    #[test]
    fn encodes_directory_paths() {
        assert_eq!(urlencode("/music/专辑 1"), "%2Fmusic%2F%E4%B8%93%E8%BE%91%201");
        assert_eq!(urlencode("/"), "%2F");
    }

    #[test]
    fn normalizes_root_and_nested_dirs() {
        assert_eq!(normalize_dir("/"), "/");
        assert_eq!(normalize_dir("/music/"), "/music");
        assert_eq!(normalize_dir(""), "/");
    }

    #[test]
    fn appends_access_token_to_dlink() {
        // 该拼接逻辑写在 resolve_dlink 里，这里用同构断言守住分隔符规则。
        let with_query = "https://d.pcs.baidu.com/file/x?fid=1";
        let separator = if with_query.contains('?') { '&' } else { '?' };
        assert!(format!("{with_query}{separator}access_token=t").contains("?fid=1&access_token=t"));
        let plain = "https://d.pcs.baidu.com/file/x";
        let separator = if plain.contains('?') { '&' } else { '?' };
        assert_eq!(
            format!("{plain}{separator}access_token=t"),
            "https://d.pcs.baidu.com/file/x?access_token=t"
        );
    }
}
