//! 飞牛音乐（fnOS Music）直连（阶段 C2）。
//!
//! 参考实现取自 primuse 的 `FnMusicAPIProtocol` / `FnMusicServiceClient`。
//! 这里**只对接飞牛音乐自身的 Web 服务**（`/music/api/v1`），不使用 fnOS 的
//! 系统 / 文件 API——曲库、账号、权限都来自音乐应用本身。
//!
//! ## 协议要点
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 登录 | `POST {base}/music/api/v1/user/password-login` |
//! | 列表 | `GET {base}/music/api/v1/track/list?page=&size=&sort=createdAt,asc` |
//! | 歌词 | `GET {base}/music/api/v1/lyric/list?trackGUID=` |
//! | 封面 | `GET {base}/music/api/v1/static/cover?coverId=&size=&t=` |
//! | 播放 | `GET {base}/music/api/v1/track/stream?guid=` |
//!
//! 除登录外的所有请求都要带两样东西：
//! 1. `Cookie: music-token=<userToken>`；
//! 2. `authx: nonce=…&timestamp=…&sign=…`（见 [`authx_header`]）。
//!
//! ## 为什么必须走本地桥接
//!
//! `authx` 的签名里含 **nonce 与毫秒时间戳**，是"一次性"凭据。播放引擎那套
//! "一个 URL + 一组静态头"的模型在这里不成立：长时间播放后再 seek，头里的
//! 时间戳早已过期。桥接把签名动作放到**每次上游请求之前**，因此始终新鲜；
//! 这与道理鱼共用 [`bridge::http_segment_opener`]。
//!
//! ## 曲目引用
//!
//! `ScannedSong.id` 是合成引用 `/fnmusic/tracks/{guid}.{ext}`，不包含飞牛的
//! 真实文件路径（服务端不暴露给客户端）。它同时是播放时的 `song_id`。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::source::bridge::{self, HttpHeaderProvider, RangeReader, SpoolReader};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::{build_blocking_client_for, build_blocking_client_with_timeout};

/// 音乐应用的固定 API 前缀。
const API_PATH: &str = "/music/api/v1";
/// 分页每页条数。
const PAGE_SIZE: usize = 50;
/// 服务端允许的单页上限。
const MAX_TAKE: usize = 500;
/// 翻页安全上限。
const MAX_PAGES: usize = 10_000;
/// 曲目引用前缀。
const REF_PREFIX: &str = "/fnmusic/tracks/";
/// 封面引用前缀。
/// `authx` 签名的固定前缀与客户端 key（浏览器端使用的同一组常量）。
const AUTHX_SIGNING_PREFIX: &str = "NDzZTVxnRKP8Z0jXg1VAMonaG8akvh";
const AUTHX_CLIENT_KEY: &str = "6D5602D4-A342-4799-A0F0-BB795E7167D0";

/// 桥接上游请求的超时（同道理鱼：不能用默认 30s 整请求上限）。
const BRIDGE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const BRIDGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

// ==================== 曲目引用 ====================

/// 合成引用：`/fnmusic/tracks/{guid}.{ext}`。
pub fn track_path(guid: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!("{REF_PREFIX}{guid}.{}", if ext.is_empty() { "bin" } else { &ext })
}

/// 从合成引用反解 guid。
pub fn track_guid(path: &str) -> Option<String> {
    let rest = path.strip_prefix(REF_PREFIX)?;
    let name = rest.rsplit('/').next().unwrap_or(rest);
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => stem,
        _ => name,
    };
    (!stem.is_empty()).then(|| stem.to_string())
}

// ==================== authx 签名 ====================

/// RFC 3986 unreserved 集合的 percent-encode。
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 规范化 query：按参数名排序（同名保持原序）后 percent-encode 拼接。
///
/// 这个字符串既是**实际发送**的 query，也是签名输入的解码来源。
pub fn canonical_query(items: &[(String, String)]) -> String {
    let mut indexed: Vec<(usize, &(String, String))> = items.iter().enumerate().collect();
    indexed.sort_by(|a, b| a.1 .0.cmp(&b.1 .0).then(a.0.cmp(&b.0)));
    indexed
        .iter()
        .map(|(_, (name, value))| format!("{}={}", percent_encode(name), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// 签名载荷。
///
/// GET 是对**规范化并解码后**的 query 做哈希——即参数名排序、去掉百分号转义；
/// 其余方法直接哈希请求体字节。
pub fn authx_payload(
    method: &str,
    query_items: &[(String, String)],
    body: Option<&[u8]>,
) -> Vec<u8> {
    if method.eq_ignore_ascii_case("GET") {
        let mut indexed: Vec<(usize, &(String, String))> = query_items.iter().enumerate().collect();
        indexed.sort_by(|a, b| a.1 .0.cmp(&b.1 .0).then(a.0.cmp(&b.0)));
        indexed
            .iter()
            .map(|(_, (name, value))| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("&")
            .into_bytes()
    } else {
        body.unwrap_or_default().to_vec()
    }
}

/// 计算 `authx` 请求头。
///
/// `sign = MD5(prefix_path_nonce_timestamp_payloadHash_key)`，
/// 其中 `payloadHash = MD5(payload)`。
#[allow(clippy::too_many_arguments)]
pub fn authx_header(
    method: &str,
    path: &str,
    query_items: &[(String, String)],
    body: Option<&[u8]>,
    nonce: &str,
    timestamp_ms: i64,
    prefix: &str,
    key: &str,
) -> String {
    let payload = authx_payload(method, query_items, body);
    let payload_hash = format!("{:x}", md5::compute(&payload));
    let timestamp = timestamp_ms.to_string();
    let source = [prefix, path, nonce, timestamp.as_str(), payload_hash.as_str(), key].join("_");
    let signature = format!("{:x}", md5::compute(source.as_bytes()));
    format!("nonce={nonce}&timestamp={timestamp}&sign={signature}")
}

/// 六位随机 nonce。
fn random_nonce() -> String {
    use rand::Rng;
    format!("{:06}", rand::thread_rng().gen_range(100_000..=999_999))
}

/// 当前 Unix 毫秒时间戳。
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 为一次请求生成新鲜的 `authx`。
fn fresh_authx(method: &str, path: &str, query: &[(String, String)], body: Option<&[u8]>) -> String {
    authx_header(
        method,
        path,
        query,
        body,
        &random_nonce(),
        now_millis(),
        AUTHX_SIGNING_PREFIX,
        AUTHX_CLIENT_KEY,
    )
}

// ==================== URL 拼接 ====================

/// API 基地址（`server_url` + 可选反代前缀 + `/music/api/v1`）。
fn api_base(config: &StreamServerConfig) -> Option<String> {
    let raw = config.server_url.trim().trim_end_matches('/');
    if raw.is_empty() {
        return None;
    }
    let prefix = config
        .base_path
        .as_deref()
        .unwrap_or("")
        .trim()
        .trim_matches('/');
    let mut base = raw.to_string();
    if !prefix.is_empty() {
        base.push('/');
        base.push_str(prefix);
    }
    Some(if base.ends_with(API_PATH) {
        base
    } else if base.ends_with("/music") {
        format!("{base}/api/v1")
    } else {
        format!("{base}{API_PATH}")
    })
}

/// `authx` 签名使用的路径。
///
/// 服务端始终按**规范 API 路径**验签，与反向代理前缀无关，因此这里固定为
/// `/music/api/v1` + 端点。
fn authx_path(endpoint: &str) -> String {
    format!("{API_PATH}{endpoint}")
}

/// 组装最终 URL（query 与实际签名内容一致）。
fn endpoint_url(
    base: &str,
    endpoint: &str,
    query_items: &[(String, String)],
) -> String {
    if query_items.is_empty() {
        format!("{base}{endpoint}")
    } else {
        format!("{base}{endpoint}?{}", canonical_query(query_items))
    }
}

// ==================== 会话 ====================

fn token_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(config: &StreamServerConfig) -> String {
    format!(
        "{}\u{1f}{}",
        config.server_url.trim(),
        config.username.trim()
    )
}

fn forget_token(config: &StreamServerConfig) {
    if let Ok(mut cache) = token_cache().lock() {
        cache.remove(&cache_key(config));
    }
}

/// 稳定的设备标识（64 位 hex 里的 32 位）。
///
/// 由服务器地址派生而非随机持久化：同一台 NAS 每次启动都会得到同一个值，
/// 无需额外存储，也不会因为重装而"变成新设备"。
fn device_id(config: &StreamServerConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"bayin-fnmusic-device\x1f");
    hasher.update(config.server_url.trim().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    digest[..32].to_string()
}

/// 密码哈希（服务端要求 SHA-256 十六进制，而非明文）。
fn password_hash(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// `Cookie` 值里的 token 需要 percent-encode（与浏览器行为一致）。
fn music_token_cookie(token: &str) -> String {
    format!("music-token={}", percent_encode(token))
}

/// 登录换取 `userToken`。
fn login(config: &StreamServerConfig) -> Result<String, String> {
    let base = api_base(config).ok_or("未填写飞牛音乐服务器地址")?;
    let username = config.username.trim();
    if username.is_empty() || config.password.is_empty() {
        return Err("未填写飞牛音乐账号或密码".to_string());
    }

    let body = serde_json::to_vec(&serde_json::json!({
        "username": username,
        "password": password_hash(&config.password),
        "deviceId": device_id(config),
    }))
    .map_err(|e| format!("序列化登录请求失败: {e}"))?;

    let path = authx_path("/user/password-login");
    let authx = fresh_authx("POST", &path, &[], Some(&body));
    let client = build_blocking_client_for(&config.server_url)?;
    let response = client
        .post(format!("{base}/user/password-login"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Accept-Language", "zh-CN")
        .header("authx", authx)
        .body(body)
        .send()
        .map_err(|e| format!("连接飞牛音乐失败: {e}"))?;

    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err("飞牛音乐账号或密码不正确".to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!("飞牛音乐登录失败（HTTP {status}）"));
    }
    let value: Value = response
        .json()
        .map_err(|e| format!("飞牛音乐登录响应不是 JSON: {e}"))?;
    ensure_business_ok(&value)?;
    value
        .get("data")
        .and_then(|d| d.get("userToken"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "飞牛音乐登录响应缺少 userToken".to_string())
}

/// 业务信封校验：`code` 为 0/200 视为成功。
fn ensure_business_ok(value: &Value) -> Result<(), String> {
    let code = value
        .get("code")
        .and_then(|c| match c {
            Value::Number(n) => n.as_i64(),
            Value::String(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(-1);
    if code == 0 || code == 200 {
        return Ok(());
    }
    if code == 120_001 || code == 401 || code == 403 {
        return Err(AUTH_FAILED.to_string());
    }
    let message = value
        .get("msg")
        .or_else(|| value.get("message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("飞牛音乐业务错误 {code}"));
    Err(message)
}

/// 认证失败的内部哨兵，供上层决定"清 token 重试"。
const AUTH_FAILED: &str = "__bayin_fnmusic_auth_failed__";

fn is_auth_failure(error: &str) -> bool {
    error == AUTH_FAILED
}

fn ensure_token(config: &StreamServerConfig) -> Result<String, String> {
    let key = cache_key(config);
    if let Some(token) = token_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(&key).cloned())
    {
        return Ok(token);
    }
    let token = login(config)?;
    if let Ok(mut cache) = token_cache().lock() {
        cache.insert(key, token.clone());
    }
    Ok(token)
}

/// 带认证的 JSON 请求（401/403 时清 token 重试一次）。
fn authorized_json(
    config: &StreamServerConfig,
    method: &str,
    endpoint: &str,
    query_items: &[(String, String)],
    body: Option<Vec<u8>>,
) -> Result<Value, String> {
    let base = api_base(config).ok_or("未填写飞牛音乐服务器地址")?;
    let path = authx_path(endpoint);
    let url = endpoint_url(&base, endpoint, query_items);
    let client = build_blocking_client_for(&config.server_url)?;

    for attempt in 0..2 {
        let token = ensure_token(config)?;
        let authx = fresh_authx(method, &path, query_items, body.as_deref());
        let mut request = client
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?,
                &url,
            )
            .header("Accept", "application/json")
            .header("Accept-Language", "zh-CN")
            .header("Cookie", music_token_cookie(&token))
            .header("authx", authx);
        if let Some(ref bytes) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(bytes.clone());
        }

        let response = request
            .send()
            .map_err(|e| format!("请求飞牛音乐失败: {e}"))?;
        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            forget_token(config);
            if attempt == 0 {
                continue;
            }
            return Err("飞牛音乐拒绝认证（账号或密码可能已变更）".to_string());
        }
        if !(200..300).contains(&status) {
            return Err(format!("飞牛音乐返回 HTTP {status}"));
        }
        let value: Value = response
            .json()
            .map_err(|e| format!("飞牛音乐响应不是 JSON: {e}"))?;
        match ensure_business_ok(&value) {
            Ok(()) => {}
            Err(e) if is_auth_failure(&e) && attempt == 0 => {
                forget_token(config);
                continue;
            }
            Err(e) if is_auth_failure(&e) => {
                return Err("飞牛音乐拒绝认证（账号或密码可能已变更）".to_string());
            }
            Err(e) => return Err(e),
        }
        return Ok(value.get("data").cloned().unwrap_or(Value::Null));
    }
    Err("飞牛音乐认证失败".to_string())
}

// ==================== 曲目解析 ====================

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

fn as_int(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn as_float(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn int_at(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| as_int(value.get(*key)))
}

/// 从 `["艺术家1","艺术家2"]` 或 `[{"name":"…"}]` 取名字。
fn object_or_string_names(value: Option<&Value>, keys: &[&str]) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => {
                    let t = s.trim();
                    (!t.is_empty()).then(|| t.to_string())
                }
                _ => first_string(item, keys),
            })
            .collect(),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t.to_string()]
            }
        }
        _ => Vec::new(),
    }
}

/// 封面 id：直接字段、`coverUrl` 的 query，或嵌套 `cover` 对象。
fn cover_id_of(value: &Value, album: Option<&Value>) -> Option<String> {
    let direct = first_string(value, &["coverId", "coverID", "coverGuid", "coverGUID"])
        .or_else(|| album.and_then(|a| first_string(a, &["coverId", "coverID", "coverGuid", "coverGUID"])));
    if direct.is_some() {
        return direct;
    }
    if let Some(cover_url) = first_string(value, &["coverUrl", "coverURL"]) {
        if let Ok(parsed) = url::Url::parse(&cover_url) {
            if let Some(id) = parsed
                .query_pairs()
                .find(|(k, _)| k == "coverId")
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty())
            {
                return Some(id);
            }
        }
    }
    value
        .get("cover")
        .and_then(|c| {
            c.as_object()
                .map(|_| c)
                .or_else(|| c.as_array().and_then(|a| a.first()))
        })
        .and_then(|c| first_string(c, &["coverId", "guid", "id"]))
}

/// 飞牛曲库条目。
#[derive(Debug, Clone, Default)]
pub struct Track {
    pub guid: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    /// 秒。
    pub duration: f64,
    pub extension: Option<String>,
    pub file_size: u64,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub cover_id: Option<String>,
}

impl Track {
    pub fn from_json(value: &Value) -> Option<Self> {
        let guid = first_string(value, &["guid", "id", "trackGUID", "trackGuid", "songId"])?;

        let album = value.get("album").filter(|v| v.is_object());
        let audio_spec = value
            .get("audioSpec")
            .filter(|v| v.is_object())
            .or_else(|| value.get("audio").filter(|v| v.is_object()));

        // 格式字段混着后缀、容器名、MIME 与编解码别名，逐个试到能识别为止。
        let extension = [
            audio_spec.and_then(|a| first_string(a, &["extension", "format", "container", "codec"])),
            first_string(value, &["extension", "format"]),
            audio_spec
                .and_then(|a| first_string(a, &["path"]))
                .and_then(extension_of),
            first_string(value, &["path"]).and_then(extension_of),
        ]
        .into_iter()
        .flatten()
        .find_map(|raw| normalize_audio_extension(&raw));

        let is_cue = value
            .get("isCue")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| first_string(value, &["cueSheet"]).is_some());
        let cue_start = as_float(value.get("startTime").or_else(|| value.get("cueStartTime")));
        let cue_end = as_float(value.get("endTime").or_else(|| value.get("cueEndTime")));

        let duration_ms = match (is_cue, cue_start, cue_end) {
            (true, Some(start), Some(end)) if end > start => ((end - start) * 1000.0).round() as i64,
            _ => {
                let from_spec = audio_spec.and_then(|a| as_int(a.get("duration"))).unwrap_or(0);
                if from_spec != 0 {
                    from_spec
                } else {
                    int_at(value, &["duration"]).unwrap_or(0)
                }
            }
        };

        let album_artist = first_string(value, &["albumArtist", "albumArtistName"])
            .or_else(|| album.and_then(|a| first_string(a, &["albumArtist", "artistName"])))
            .or_else(|| {
                album.and_then(|a| a.get("artist"))
                    .and_then(|artist| match artist {
                        Value::String(s) => {
                            let t = s.trim();
                            (!t.is_empty()).then(|| t.to_string())
                        }
                        _ => first_string(artist, &["name", "title"]),
                    })
            })
            .or_else(|| {
                let names = object_or_string_names(
                    value.get("albumArtists").or_else(|| album.and_then(|a| a.get("artists"))),
                    &["name", "title"],
                );
                (!names.is_empty()).then(|| names.join(", "))
            });

        let artist_names = object_or_string_names(value.get("artists"), &["name", "title"]);
        let artist = if artist_names.is_empty() {
            first_string(value, &["artist"]).unwrap_or_default()
        } else {
            artist_names.join(", ")
        };

        let bitrate_raw =
            audio_spec.and_then(|a| as_int(a.get("bitrate"))).or_else(|| int_at(value, &["bitrate"]));
        // 服务端有时给 bps、有时给 kbps；≥10000 一律按 bps 折算。
        let bitrate = bitrate_raw
            .map(|b| if b >= 10_000 { (b / 1000).max(1) } else { b })
            .and_then(|b| u32::try_from(b).ok());

        Some(Track {
            guid,
            title: first_string(value, &["title", "trackTitle", "name"])
                .or_else(|| first_string(value, &["filename"]))
                .unwrap_or_else(|| "未知标题".to_string()),
            artist,
            album: first_string(value, &["albumTitle"])
                .or_else(|| album.and_then(|a| first_string(a, &["name", "title"])))
                .or_else(|| first_string(value, &["album"]))
                .unwrap_or_default(),
            album_artist: album_artist.unwrap_or_default(),
            duration: duration_ms as f64 / 1000.0,
            extension,
            file_size: audio_spec
                .and_then(|a| as_int(a.get("size")))
                .or_else(|| int_at(value, &["size"]))
                .unwrap_or(0)
                .max(0) as u64,
            bitrate,
            sample_rate: audio_spec
                .and_then(|a| as_int(a.get("sampleRate")))
                .or_else(|| int_at(value, &["sampleRate"]))
                .and_then(|v| u32::try_from(v).ok()),
            bit_depth: audio_spec
                .and_then(|a| as_int(a.get("bitDepth")))
                .or_else(|| int_at(value, &["bitDepth"]))
                .and_then(|v| u8::try_from(v).ok()),
            created_at: int_at(value, &["createdAt"]),
            updated_at: int_at(value, &["updatedAt"]),
            cover_id: cover_id_of(value, album),
        })
    }

    /// 转成库内记录（不含封面字节；封面另走 [`fetch_cover`] 落盘缓存）。
    pub fn to_scanned_song(&self) -> Option<ScannedSong> {
        let ext = self.extension.clone()?;
        let reference = track_path(&self.guid, &ext);
        let is_hr = self.bit_depth.map(|d| d > 16).unwrap_or(false)
            || self.sample_rate.map(|r| r > 44_100).unwrap_or(false);
        let revision = self.updated_at.or(self.created_at);

        Some(ScannedSong {
            id: reference.clone(),
            title: self.title.clone(),
            artist: if self.artist.is_empty() {
                self.album_artist.clone()
            } else {
                self.artist.clone()
            },
            album: self.album.clone(),
            duration: self.duration,
            file_path: reference,
            file_size: self.file_size,
            // 封面需要 Cookie + authx，`<img>` 带不了，因此交给本地缓存（cover_hash）。
            cover_url: None,
            cover_hash: None,
            file_modified: revision,
            is_hr: Some(is_hr),
            is_sq: Some(is_lossless_extension(&ext)),
            format: Some(ext.to_ascii_uppercase()),
            bit_depth: self.bit_depth,
            sample_rate: self.sample_rate,
            bitrate: self.bitrate,
            channels: None,
            created_at: self.created_at,
        })
    }

    /// `(coverId, revision)`。
    pub fn cover_ref(&self) -> Option<(String, Option<i64>)> {
        self.cover_id
            .clone()
            .map(|id| (id, self.updated_at.or(self.created_at)))
    }
}

fn extension_of(path: String) -> Option<String> {
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.').map(|(_, ext)| ext.to_string()))
}

// ==================== 分页 ====================

pub struct Page {
    pub tracks: Vec<Track>,
    pub total: Option<u64>,
    pub raw_count: usize,
}

/// 解析 `{list: [...], total}`。
pub fn parse_page(value: &Value) -> Result<Page, String> {
    let items = value.get("list").and_then(Value::as_array).ok_or("飞牛音乐响应缺少 list")?;
    let total = as_int(value.get("total")).map(|t| t.max(0) as u64);
    let tracks: Vec<Track> = items.iter().filter_map(Track::from_json).collect();
    Ok(Page {
        tracks,
        total,
        raw_count: items.len(),
    })
}

fn fetch_page(config: &StreamServerConfig, page: u64, size: usize) -> Result<Page, String> {
    if size == 0 || size > MAX_TAKE {
        return Err(format!("每页条数必须在 1..={MAX_TAKE}"));
    }
    let query = vec![
        ("page".to_string(), page.to_string()),
        ("size".to_string(), size.to_string()),
        ("sort".to_string(), "createdAt,asc".to_string()),
    ];
    let data = authorized_json(config, "GET", "/track/list", &query, None)?;
    parse_page(&data)
}

// ==================== 扫描 ====================

/// 拉取整库。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    _existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let mut songs: Vec<ScannedSong> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut page: u64 = 1;
    let mut received: u64 = 0;
    let mut expected_total: Option<u64> = None;

    for page_index in 0..MAX_PAGES {
        let result = fetch_page(config, page, PAGE_SIZE)?;

        let total = result
            .total
            .ok_or("飞牛音乐响应缺少 total，无法确认扫描完整")?;
        if let Some(expected) = expected_total {
            if expected != total {
                return Err("飞牛音乐曲库总数在扫描过程中发生变化，请重试".to_string());
            }
        }
        expected_total = Some(total);

        if result.raw_count > PAGE_SIZE {
            return Err("飞牛音乐单页返回条数超过请求值".to_string());
        }
        if total == 0 {
            if page != 1 || result.raw_count != 0 {
                return Err("飞牛音乐分页与曲库总数不一致".to_string());
            }
            break;
        }
        if result.raw_count == 0 {
            return Err("飞牛音乐分页提前结束".to_string());
        }

        received += result.raw_count as u64;
        for track in &result.tracks {
            if !seen.insert(track.guid.clone()) {
                return Err(format!("飞牛曲库出现重复曲目: {}", track.guid));
            }
            // 格式不可识别 → 跳过（解码能力差异，不算协议违背）。
            if let Some(mut song) = track.to_scanned_song() {
                // 封面按需下载：只有深度扫描（read_tags）才付出全库拉封面的代价。
                if read_tags {
                    if let (Some(cache), Some((cover_id, revision))) = (cover_cache, track.cover_ref())
                    {
                        if let Ok(bytes) = fetch_cover(config, &cover_id, revision) {
                            if let Ok(hash) = cache.save_cover(&bytes, None) {
                                song.cover_hash = Some(hash);
                            }
                        }
                    }
                }
                songs.push(song);
            } else {
                log::debug!("[飞牛音乐] 跳过无可用格式的曲目: {} ({:?})", track.title, track.extension);
            }
        }

        if received > total {
            return Err("飞牛音乐分页越过曲库末尾".to_string());
        }
        if let Some(tx) = &progress {
            let _ = tx.send((songs.len(), total as usize));
        }
        if received == total {
            break;
        }
        if result.raw_count != PAGE_SIZE {
            return Err("飞牛音乐分页未按预期满页返回".to_string());
        }
        page += 1;

        if page_index + 1 == MAX_PAGES {
            return Err("飞牛音乐曲库分页超过安全上限".to_string());
        }
    }

    if let Some(tx) = &progress {
        let total = songs.len();
        let _ = tx.send((total, total));
    }
    Ok(songs)
}

/// 连接测试：登录 + 拉一页。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if api_base(config).is_none() {
        return ConnectionTestResult {
            success: false,
            message: "未填写飞牛音乐服务器地址".to_string(),
            server_version: None,
        };
    }
    match fetch_page(config, 1, 1) {
        Ok(result) => ConnectionTestResult {
            success: true,
            message: match result.total {
                Some(total) => format!("已连接到飞牛音乐（曲库 {total} 首）"),
                None => "已连接到飞牛音乐".to_string(),
            },
            server_version: None,
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

// ==================== 封面 / 歌词 ====================

/// 拉取封面字节。
pub fn fetch_cover(
    config: &StreamServerConfig,
    cover_id: &str,
    revision: Option<i64>,
) -> Result<Vec<u8>, String> {
    let mut query = vec![
        ("coverId".to_string(), cover_id.to_string()),
        ("size".to_string(), "640".to_string()),
    ];
    if let Some(rev) = revision {
        query.push(("t".to_string(), rev.to_string()));
    }
    let base = api_base(config).ok_or("未填写飞牛音乐服务器地址")?;
    let token = ensure_token(config)?;
    let path = authx_path("/static/cover");
    let authx = fresh_authx("GET", &path, &query, None);
    let url = endpoint_url(&base, "/static/cover", &query);

    let client = build_blocking_client_for(&config.server_url)?;
    let mut response = client
        .get(&url)
        .header("Cookie", music_token_cookie(&token))
        .header("authx", authx)
        .send()
        .map_err(|e| format!("请求飞牛音乐封面失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("飞牛音乐封面返回 HTTP {}", response.status().as_u16()));
    }
    let mut buffer = Vec::new();
    response
        .copy_to(&mut buffer)
        .map_err(|e| format!("读取飞牛音乐封面失败: {e}"))?;
    Ok(buffer)
}

/// 歌词：取 `list` 中 `preferred` 指定的那条，否则取第一条。
pub fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let guid = track_guid(song_id)?;
    let query = vec![("trackGUID".to_string(), guid)];
    let data = authorized_json(config, "GET", "/lyric/list", &query, None).ok()?;

    let preferred = data
        .get("preferred")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let items = data
        .get("list")
        .and_then(Value::as_array)
        .or_else(|| data.as_array())?;

    let collect = |value: &Value| -> Option<(String, String)> {
        let content = first_string(value, &["content", "text"])?;
        let id = first_string(value, &["guid", "id"]).unwrap_or_default();
        Some((id, content))
    };

    let parsed: Vec<(String, String)> = items.iter().filter_map(collect).collect();
    if let Some(preferred) = preferred {
        if let Some(hit) = parsed.iter().find(|(id, _)| id == preferred) {
            return Some(hit.1.clone());
        }
    }
    parsed.first().map(|(_, content)| content.clone())
}

// ==================== 播放 ====================

/// 生成播放 URL（经本地桥接；`authx` 由桥接在每次请求前重签）。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    match bridge::register(config, song_id) {
        Ok(url) => url,
        Err(e) => {
            log::error!("[飞牛音乐] 建立本地桥接失败: {e}");
            String::new()
        }
    }
}

/// 构造可随机读取的桥接读取器。
pub fn open_reader(
    config: &StreamServerConfig,
    song_id: &str,
) -> Result<Box<dyn RangeReader>, String> {
    let guid = track_guid(song_id).ok_or_else(|| format!("无法识别的飞牛曲目引用: {song_id}"))?;
    let base = api_base(config).ok_or("未填写飞牛音乐服务器地址")?;
    let query = vec![("guid".to_string(), guid)];
    let url = endpoint_url(&base, "/track/stream", &query);

    let client = build_blocking_client_with_timeout(
        &config.server_url,
        BRIDGE_CONNECT_TIMEOUT,
        BRIDGE_REQUEST_TIMEOUT,
    )?;

    let owned = config.clone();
    let stream_path = authx_path("/track/stream");
    let stream_query = query.clone();
    let headers: Arc<HttpHeaderProvider> = Arc::new(move || {
        // 关键：**每次**上游请求都重新登录检查 + 重新签名。
        let token = ensure_token(&owned)?;
        let authx = fresh_authx("GET", &stream_path, &stream_query, None);
        Ok(vec![
            ("Cookie".to_string(), music_token_cookie(&token)),
            ("authx".to_string(), authx),
        ])
    });

    let total = bridge::probe_total_len(&client, &url, headers.as_ref())?;
    let opener = bridge::http_segment_opener(client, url, headers);
    Ok(Box::new(SpoolReader::new(total, opener)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> StreamServerConfig {
        StreamServerConfig {
            server_type: crate::models::ServerType::FnMusic,
            server_name: "fn".to_string(),
            server_url: "http://192.168.1.5:5666".to_string(),
            username: "admin".to_string(),
            password: "secret".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        }
    }

    #[test]
    fn track_reference_round_trip() {
        assert_eq!(track_path("abc-123", "FLAC"), "/fnmusic/tracks/abc-123.flac");
        assert_eq!(
            track_guid("/fnmusic/tracks/abc-123.flac").as_deref(),
            Some("abc-123")
        );
        assert_eq!(track_guid("/fnmusic/tracks/x.bin").as_deref(), Some("x"));
        assert_eq!(track_guid("/other/tracks/x.flac"), None);
        assert_eq!(track_guid("/fnmusic/tracks/"), None);
    }

    #[test]
    fn api_base_appends_fixed_prefix_and_keeps_reverse_proxy_prefix() {
        let mut cfg = config();
        assert_eq!(
            api_base(&cfg).as_deref(),
            Some("http://192.168.1.5:5666/music/api/v1")
        );

        cfg.base_path = Some("/proxy/".to_string());
        assert_eq!(
            api_base(&cfg).as_deref(),
            Some("http://192.168.1.5:5666/proxy/music/api/v1")
        );

        // 用户把 /music 一起填进地址时只补 /api/v1。
        cfg.base_path = None;
        cfg.server_url = "http://nas.local/music".to_string();
        assert_eq!(
            api_base(&cfg).as_deref(),
            Some("http://nas.local/music/api/v1")
        );

        cfg.server_url = "http://nas.local/music/api/v1".to_string();
        assert_eq!(
            api_base(&cfg).as_deref(),
            Some("http://nas.local/music/api/v1")
        );
    }

    #[test]
    fn authx_path_is_independent_of_reverse_proxy_prefix() {
        assert_eq!(authx_path("/track/stream"), "/music/api/v1/track/stream");
    }

    #[test]
    fn percent_encode_matches_rfc3986_unreserved() {
        assert_eq!(percent_encode("abc-._~"), "abc-._~");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("createdAt,asc"), "createdAt%2Casc");
        assert_eq!(percent_encode("/x?y=1"), "%2Fx%3Fy%3D1");
    }

    #[test]
    fn canonical_query_sorts_by_name_then_original_order() {
        let items = vec![
            ("sort".to_string(), "createdAt,asc".to_string()),
            ("page".to_string(), "1".to_string()),
            ("size".to_string(), "50".to_string()),
        ];
        assert_eq!(
            canonical_query(&items),
            "page=1&size=50&sort=createdAt%2Casc"
        );
    }

    #[test]
    fn authx_payload_uses_sorted_decoded_query_for_get_and_body_otherwise() {
        let items = vec![
            ("sort".to_string(), "createdAt,asc".to_string()),
            ("page".to_string(), "1".to_string()),
        ];
        // GET：排序、解码（无百分号转义）。
        assert_eq!(
            authx_payload("GET", &items, None),
            b"page=1&sort=createdAt,asc".to_vec()
        );
        // 非 GET：直接哈希请求体。
        assert_eq!(
            authx_payload("POST", &items, Some(b"{\"a\":1}")),
            b"{\"a\":1}".to_vec()
        );
        assert_eq!(authx_payload("POST", &items, None), Vec::<u8>::new());
    }

    #[test]
    fn authx_header_is_deterministic_and_well_formed() {
        let items = vec![("guid".to_string(), "g1".to_string())];
        let header = authx_header(
            "GET",
            "/music/api/v1/track/stream",
            &items,
            None,
            "123456",
            1_700_000_000_000,
            AUTHX_SIGNING_PREFIX,
            AUTHX_CLIENT_KEY,
        );
        assert!(header.starts_with("nonce=123456&timestamp=1700000000000&sign="));
        let sign = header.rsplit("sign=").next().unwrap();
        assert_eq!(sign.len(), 32);
        assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));

        // 相同输入 → 相同签名。
        let again = authx_header(
            "GET",
            "/music/api/v1/track/stream",
            &items,
            None,
            "123456",
            1_700_000_000_000,
            AUTHX_SIGNING_PREFIX,
            AUTHX_CLIENT_KEY,
        );
        assert_eq!(header, again);

        // 换 nonce / 时间戳 / 路径 → 签名变化。
        let other = authx_header(
            "GET",
            "/music/api/v1/track/stream",
            &items,
            None,
            "654321",
            1_700_000_000_000,
            AUTHX_SIGNING_PREFIX,
            AUTHX_CLIENT_KEY,
        );
        assert_ne!(header, other);
        let other_path = authx_header(
            "GET",
            "/music/api/v1/lyric/list",
            &items,
            None,
            "123456",
            1_700_000_000_000,
            AUTHX_SIGNING_PREFIX,
            AUTHX_CLIENT_KEY,
        );
        assert_ne!(header, other_path);
    }

    #[test]
    fn authx_signature_matches_manual_composition() {
        // 用独立的一步步拼接验证签名公式，而不是复用实现内部的组合方式。
        let payload = b"guid=g1".to_vec();
        let payload_hash = format!("{:x}", md5::compute(&payload));
        let source = format!(
            "{AUTHX_SIGNING_PREFIX}_/music/api/v1/track/stream_123456_1700000000000_{payload_hash}_{AUTHX_CLIENT_KEY}"
        );
        let expected = format!("{:x}", md5::compute(source.as_bytes()));

        let header = authx_header(
            "GET",
            "/music/api/v1/track/stream",
            &[("guid".to_string(), "g1".to_string())],
            None,
            "123456",
            1_700_000_000_000,
            AUTHX_SIGNING_PREFIX,
            AUTHX_CLIENT_KEY,
        );
        assert_eq!(header.rsplit("sign=").next().unwrap(), expected);
    }

    #[test]
    fn parses_catalog_track_payload() {
        let json = serde_json::json!({
            "guid": "g-1",
            "title": "Feiniu Tone",
            "album": { "guid": "al-1", "name": "Album", "albumArtist": "VA" },
            "artists": [{ "guid": "ar-1", "name": "Artist A" }, { "guid": "ar-2", "name": "Artist B" }],
            "genres": [{ "name": "Rock" }],
            "year": 2024,
            "trackNo": 3,
            "discNo": 1,
            "audioSpec": {
                "duration": 245000,
                "extension": "flac",
                "size": 30_000_000,
                "bitrate": 1_411_200,
                "sampleRate": 96000,
                "bitDepth": 24
            },
            "coverId": "cov-1",
            "createdAt": 1_700_000_000,
            "updatedAt": 1_700_000_500
        });
        let track = Track::from_json(&json).expect("应能解析");
        assert_eq!(track.guid, "g-1");
        assert_eq!(track.title, "Feiniu Tone");
        assert_eq!(track.artist, "Artist A, Artist B");
        assert_eq!(track.album, "Album");
        assert_eq!(track.album_artist, "VA");
        assert_eq!(track.duration, 245.0);
        assert_eq!(track.extension.as_deref(), Some("flac"));
        assert_eq!(track.file_size, 30_000_000);
        assert_eq!(track.bitrate, Some(1411));
        assert_eq!(track.sample_rate, Some(96000));
        assert_eq!(track.bit_depth, Some(24));

        let song = track.to_scanned_song().expect("应能转 song");
        assert_eq!(song.id, "/fnmusic/tracks/g-1.flac");
        assert_eq!(song.duration, 245.0);
        assert_eq!(song.is_sq, Some(true));
        assert_eq!(song.is_hr, Some(true));
        assert_eq!(song.format.as_deref(), Some("FLAC"));
        assert_eq!(song.cover_hash, None);
        assert_eq!(
            track.cover_ref(),
            Some(("cov-1".to_string(), Some(1_700_000_500)))
        );
    }

    #[test]
    fn cue_range_overrides_audio_spec_duration() {
        let json = serde_json::json!({
            "guid": "g-cue",
            "title": "Cue Track",
            "isCue": true,
            "startTime": 10.0,
            "endTime": 40.5,
            "audioSpec": { "duration": 1_800_000, "extension": "flac" }
        });
        let track = Track::from_json(&json).expect("应能解析");
        assert_eq!(track.duration, 30.5);
    }

    #[test]
    fn bitrate_below_ten_thousand_is_kept_as_is() {
        let json = serde_json::json!({
            "guid": "g2", "title": "x",
            "audioSpec": { "extension": "mp3", "bitrate": 320 }
        });
        let track = Track::from_json(&json).expect("应能解析");
        assert_eq!(track.bitrate, Some(320));
    }

    #[test]
    fn unknown_format_is_skipped_not_registered() {
        let json = serde_json::json!({
            "guid": "g3", "title": "x",
            "audioSpec": { "extension": "exe", "duration": 1000 }
        });
        let track = Track::from_json(&json).expect("元数据仍应解析");
        assert_eq!(track.extension, None);
        assert!(track.to_scanned_song().is_none());
    }

    #[test]
    fn page_requires_list_and_reports_total() {
        let value = serde_json::json!({
            "list": [{"guid": "a", "audioSpec": {"extension": "flac"}}],
            "total": 1
        });
        let page = parse_page(&value).expect("应能解析");
        assert_eq!(page.total, Some(1));
        assert_eq!(page.raw_count, 1);
        assert_eq!(page.tracks.len(), 1);

        assert!(parse_page(&serde_json::json!({ "total": 0 })).is_err());
    }

    #[test]
    fn business_envelope_distinguishes_auth_failure() {
        assert!(ensure_business_ok(&serde_json::json!({"code": 0})).is_ok());
        assert!(ensure_business_ok(&serde_json::json!({"code": 200})).is_ok());

        let auth = ensure_business_ok(&serde_json::json!({"code": 120001})).unwrap_err();
        assert!(is_auth_failure(&auth));

        let other = ensure_business_ok(&serde_json::json!({"code": 500, "msg": "boom"})).unwrap_err();
        assert_eq!(other, "boom");
        assert!(!is_auth_failure(&other));
    }

    #[test]
    fn device_id_is_stable_and_32_hex() {
        let cfg = config();
        let first = device_id(&cfg);
        assert_eq!(first.len(), 32);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(first, device_id(&cfg));
        // 不同服务器 → 不同设备标识。
        let mut other = cfg.clone();
        other.server_url = "http://192.168.1.6:5666".to_string();
        assert_ne!(first, device_id(&other));
    }

    #[test]
    fn password_hash_is_sha256_hex() {
        let hash = password_hash("secret");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        // SHA-256("secret") 的已知值。
        assert_eq!(
            hash,
            "2bb80d537b1da3e38bd30361aa855686bde0eacd7162fef6a25fe97bf527a25b"
        );
    }

    #[test]
    fn token_cookie_percent_encodes_value() {
        assert_eq!(music_token_cookie("abc.DEF-123"), "music-token=abc.DEF-123");
        assert_eq!(music_token_cookie("a b"), "music-token=a%20b");
    }
}
