//! 道理鱼（DaoLiYu）音乐服务直连（阶段 C3）。
//!
//! 形态与 Jellyfin/Plex 同族：**只读整库**，没有文件系统语义——曲目由服务端
//! 曲库表统一给出，扫描即"翻页拉全表"。参考实现取自 primuse 的
//! `DaoLiYuAPIProtocol` / `DaoLiYuServiceClient`。
//!
//! ## 协议要点
//!
//! | 用途 | 请求 |
//! |---|---|
//! | 登录 | `POST {base}/api/auth/login`，body `{email,username,password}` → `{token}` |
//! | 分页 | `GET {base}/api/tracks?skip=&take=` → `{items,total,skip,take}` |
//! | 详情 | `GET {base}/api/tracks/{id}` → 含歌词 |
//! | 播放 | `GET {base}/api/tracks/{id}/download` + `Authorization: Bearer` |
//! | 封面 | `GET {base}/api/cover?path=<ref>`（无需认证） |
//!
//! ## 为什么走本地桥接，而不是像 S3 那样直接给播放引擎
//!
//! 播放端点必须携带 `Authorization: Bearer`。给播放引擎一个"URL + 静态头"
//! 虽然也能跑，但令牌一旦过期，A5 Range 写透与 seek 都会静默 401。桥接把
//! 取令牌的动作放到**每次上游请求前**，令牌失效可当场重登重试，播放不断流。
//! 这与飞牛音乐（`authx` 含时间戳）共用同一套 [`bridge::http_segment_opener`]。
//!
//! ## 曲目引用
//!
//! 库内的 `ScannedSong.id` 是合成引用 `/daoliyu/tracks/{track_id}.{ext}`：
//! 它同时充当"播放时的 song_id"（`commands/streaming.rs` 会把 `s.id` 写进
//! `stream_info.songId`），因此必须自带可反解信息，且不能是服务端文件路径。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::source::bridge::{self, HttpHeaderProvider, RangeReader, SpoolReader};
use crate::utils::audio::{is_lossless_extension, normalize_audio_extension};
use crate::utils::http::{build_blocking_client_for, build_blocking_client_with_timeout};

/// API 前缀。
const API_PREFIX: &str = "/api";
/// 分页每页条数。
const PAGE_SIZE: usize = 100;
/// 服务端允许的单页上限。
const MAX_TAKE: usize = 500;
/// 翻页安全上限：防止异常的 `total` 导致无限翻页。
const MAX_PAGES: usize = 10_000;
/// 曲目引用前缀。
const REF_PREFIX: &str = "/daoliyu/tracks/";

/// 桥接上游请求的超时。
///
/// 注意不能用 [`build_blocking_client_for`] 的默认 30s：那是**整请求**上限
/// （含读 body），而桥接的分段拉取是"从偏移一路读到文件末尾"，长曲目必然超时。
/// 这里只保留连接超时，把总时长放宽到 6 小时。
const BRIDGE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const BRIDGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

// ==================== 曲目引用 ====================

/// 合成引用：`/daoliyu/tracks/{id}.{ext}`。
pub fn track_path(id: &str, ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    format!("{REF_PREFIX}{id}.{}", if ext.is_empty() { "bin" } else { &ext })
}

/// 从合成引用反解曲目 id。
pub fn track_id(path: &str) -> Option<String> {
    let rest = path.strip_prefix(REF_PREFIX)?;
    let name = rest.rsplit('/').next().unwrap_or(rest);
    let (stem, _) = match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => (stem, ext),
        _ => (name, ""),
    };
    (!stem.is_empty()).then(|| stem.to_string())
}

// ==================== URL 拼接 ====================

/// 站点根地址（`server_url` + 可选 `base_path`，不含 `/api`）。
fn root_url(config: &StreamServerConfig) -> Option<String> {
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
    // 用户可能直接把 `/api` 填进了地址或基础路径，避免拼出 `/api/api`。
    let base = base.trim_end_matches('/').to_string();
    Some(match base.strip_suffix(API_PREFIX) {
        Some(head) => head.to_string(),
        None => base,
    })
}

/// API 基地址。
fn api_url(config: &StreamServerConfig) -> Option<String> {
    root_url(config).map(|root| format!("{root}{API_PREFIX}"))
}

/// 封面绝对地址。
///
/// 服务端可能给绝对 URL、`/api/...` 路径，或需要走 `/api/cover?path=` 的相对引用，
/// 三种形态各自处理（与参考实现一致）。
fn cover_url(config: &StreamServerConfig, reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return Some(reference.to_string());
    }
    let root = root_url(config)?;
    if reference.starts_with(API_PREFIX) {
        return Some(format!("{root}{reference}"));
    }
    let encoded: String = url::form_urlencoded::byte_serialize(reference.as_bytes()).collect();
    Some(format!("{root}{API_PREFIX}/cover?path={encoded}"))
}

// ==================== 会话（token 缓存） ====================

fn token_cache() -> &'static Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 缓存键：同一台服务器 + 同一账号复用 token。
///
/// 刻意不含密码——密钥泄露面更小；密码若被改动，旧 token 会 401，
/// 届时清缓存重登即可（见 [`authorized_get`] 的重试分支）。
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

/// 登录换取 token（`POST /api/auth/login`）。
fn login(config: &StreamServerConfig) -> Result<String, String> {
    let url = format!(
        "{}/auth/login",
        api_url(config).ok_or("未填写道理鱼服务器地址")?
    );
    let username = config.username.trim();
    if username.is_empty() || config.password.is_empty() {
        return Err("未填写道理鱼账号或密码".to_string());
    }

    let client = build_blocking_client_for(&config.server_url)?;
    let body = serde_json::json!({
        "email": username,
        "username": username,
        "password": config.password,
    });
    let response = client
        .post(&url)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("连接道理鱼失败: {e}"))?;

    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err("道理鱼账号或密码不正确".to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!("道理鱼登录失败（HTTP {status}）"));
    }
    let value: Value = response
        .json()
        .map_err(|e| format!("道理鱼登录响应不是 JSON: {e}"))?;
    value
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "道理鱼登录响应缺少 token".to_string())
}

/// 取 token（优先缓存）。
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

/// 带认证 GET，401/403 时清缓存重登**重试一次**。
fn authorized_get(config: &StreamServerConfig, url: &str) -> Result<Value, String> {
    let client = build_blocking_client_for(&config.server_url)?;
    for attempt in 0..2 {
        let token = ensure_token(config)?;
        let response = client
            .get(url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .map_err(|e| format!("请求道理鱼失败: {e}"))?;

        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            forget_token(config);
            if attempt == 0 {
                continue;
            }
            return Err("道理鱼拒绝认证（账号或密码可能已变更）".to_string());
        }
        if !(200..300).contains(&status) {
            return Err(format!("道理鱼返回 HTTP {status}"));
        }
        return response
            .json()
            .map_err(|e| format!("道理鱼响应不是 JSON: {e}"));
    }
    Err("道理鱼认证失败".to_string())
}

/// 桥接用的认证头提供者：**每次请求前**取一次 token。
fn bearer_headers(config: &StreamServerConfig) -> Result<Vec<(String, String)>, String> {
    let token = ensure_token(config)?;
    Ok(vec![(
        "Authorization".to_string(),
        format!("Bearer {token}"),
    )])
}

// ==================== 曲目解析 ====================

/// 道理鱼曲库条目（字段对齐 primuse 的 `DaoLiYuCatalogTrack`，保留常见别名）。
#[derive(Debug, Clone, Default)]
pub struct Track {
    pub id: String,
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
    pub channels: Option<u8>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub cover_reference: Option<String>,
    pub lyrics_sync: Option<String>,
    pub lyrics_plain: Option<String>,
}

/// 取第一个非空字符串。
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

/// 宽松取整数（数字或数字字符串）。
fn as_int(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 宽松取浮点。
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

fn float_at(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| as_float(value.get(*key)))
}

/// 时间戳归一成 unix 秒（兼容秒 / 毫秒 / ISO 8601 字符串）。
fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_f64().map(|f| {
            if f > 10_000_000_000.0 {
                (f / 1000.0) as i64
            } else {
                f as i64
            }
        }),
        Value::String(s) => {
            let s = s.trim();
            if let Ok(raw) = s.parse::<f64>() {
                return Some(if raw > 10_000_000_000.0 {
                    (raw / 1000.0) as i64
                } else {
                    raw as i64
                });
            }
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp())
        }
        _ => None,
    }
}

/// 歌词字段可能是字符串，也可能是 `{content|text|lrc}` 对象。
fn lyrics_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(text.to_string());
    }
    first_string(value, &["content", "text", "lrc"])
}

impl Track {
    /// 从 API JSON 解析；缺少 `id` 视为无法识别。
    pub fn from_json(value: &Value) -> Option<Self> {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())?
            .to_string();

        let album = value.get("album").filter(|v| v.is_object());
        let artist = value.get("artist").filter(|v| v.is_object());
        let artists = value
            .get("artists")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let artist_names: Vec<String> = artists
            .iter()
            .filter_map(|a| first_string(a, &["name", "title"]))
            .collect();
        let artist_name = first_string(value, &["artistName", "artistLabel"])
            .or_else(|| album.and_then(|a| first_string(a, &["artistName"])))
            .or_else(|| artist.and_then(|a| first_string(a, &["name", "title"])))
            .or_else(|| (!artist_names.is_empty()).then(|| artist_names.join(", ")))
            .unwrap_or_default();

        let album_artist = first_string(value, &["albumArtist"])
            .or_else(|| album.and_then(|a| first_string(a, &["albumArtist"])));

        let extension = [
            first_string(value, &["fileFormat", "detectedContainer"]),
            first_string(value, &["filePath"]).and_then(|p| {
                p.rsplit('/')
                    .next()
                    .and_then(|name| name.rsplit_once('.').map(|(_, e)| e.to_string()))
            }),
        ]
        .into_iter()
        .flatten()
        .find_map(|raw| normalize_audio_extension(&raw));

        let cover_reference = first_string(value, &["coverArtUrl", "coverArtPath"]).or_else(|| {
            album.and_then(|a| first_string(a, &["coverArtUrl", "coverArtPath"]))
        });

        let duration = float_at(value, &["durationSeconds", "duration"]).unwrap_or(0.0);
        let bit_depth = int_at(value, &["bitDepth"]).and_then(|v| u8::try_from(v).ok());
        let channels = int_at(value, &["channels"]).and_then(|v| u8::try_from(v).ok());

        Some(Track {
            id,
            title: first_string(value, &["title"]).unwrap_or_else(|| "未知标题".to_string()),
            artist: artist_name,
            album: first_string(value, &["albumTitle"])
                .or_else(|| album.and_then(|a| first_string(a, &["title"])))
                .unwrap_or_default(),
            album_artist: album_artist.unwrap_or_default(),
            duration,
            extension,
            file_size: int_at(value, &["fileSize"]).unwrap_or(0).max(0) as u64,
            bitrate: int_at(value, &["bitrate", "bitRate"]).and_then(|v| u32::try_from(v).ok()),
            sample_rate: int_at(value, &["sampleRate"]).and_then(|v| u32::try_from(v).ok()),
            bit_depth,
            channels,
            created_at: parse_timestamp(value.get("createdAt")),
            updated_at: parse_timestamp(value.get("updatedAt")),
            cover_reference,
            lyrics_sync: lyrics_text(value.get("lyricsSync")),
            lyrics_plain: lyrics_text(value.get("lyrics")),
        })
    }

    /// 转为库内记录。格式不可识别时返回 `None`（不把播不了的曲目登记进库）。
    pub fn to_scanned_song(
        &self,
        config: &StreamServerConfig,
        cover_cache: Option<&crate::utils::cover::CoverCache>,
    ) -> Option<ScannedSong> {
        let ext = self.extension.clone()?;
        let reference = track_path(&self.id, &ext);
        let is_hr = self.bit_depth.map(|d| d > 16).unwrap_or(false)
            || self.sample_rate.map(|r| r > 44_100).unwrap_or(false);

        // 封面在扫描期**不**下载：/api/cover 免认证，直接交给 <img> 加载即可，
        // 省掉全库拉取的代价。
        let cover = self
            .cover_reference
            .as_deref()
            .and_then(|r| cover_url(config, r));
        let _ = cover_cache;

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
            cover_url: cover,
            cover_hash: None,
            file_modified: self.updated_at.or(self.created_at),
            is_hr: Some(is_hr),
            is_sq: Some(is_lossless_extension(&ext)),
            format: Some(ext.to_ascii_uppercase()),
            bit_depth: self.bit_depth,
            sample_rate: self.sample_rate,
            bitrate: self.bitrate,
            channels: self.channels,
            created_at: self.created_at,
        })
    }
}

// ==================== 分页响应 ====================

/// 一页曲目 + 服务端给出的分页信息。
pub struct Page {
    pub tracks: Vec<Track>,
    pub total: u64,
    /// 服务端**原始**条目数（用于校验"有没有条目被静默丢弃"）。
    pub raw_count: usize,
}

/// 解析分页响应。
pub fn parse_page(value: &Value) -> Result<Page, String> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or("道理鱼分页响应缺少 items")?;
    let total = as_int(value.get("total")).ok_or("道理鱼分页响应缺少 total")?;
    if total < 0 {
        return Err("道理鱼分页响应 total 为负".to_string());
    }
    let tracks: Vec<Track> = items.iter().filter_map(Track::from_json).collect();
    Ok(Page {
        tracks,
        total: total as u64,
        raw_count: items.len(),
    })
}

/// 拉一页。
fn fetch_page(config: &StreamServerConfig, skip: u64, take: usize) -> Result<Page, String> {
    if take == 0 || take > MAX_TAKE {
        return Err(format!("每页条数必须在 1..={MAX_TAKE}"));
    }
    let url = format!(
        "{}/tracks?skip={skip}&take={take}",
        api_url(config).ok_or("未填写道理鱼服务器地址")?
    );
    parse_page(&authorized_get(config, &url)?)
}

// ==================== 扫描 ====================

/// 拉取整库。
///
/// 分页做严格校验（与参考实现一致）：`total` 前后一致、`skip` 对齐、
/// 每页条目数不超过请求值、曲目 id 不重复——任何一条不满足都说明服务端行为
/// 与协议不符，继续翻页只会得到错误结果。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    _existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    _read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let mut songs: Vec<ScannedSong> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut skip: u64 = 0;
    let mut expected_total: Option<u64> = None;

    for page_index in 0..MAX_PAGES {
        let page = fetch_page(config, skip, PAGE_SIZE)?;

        if let Some(total) = expected_total {
            if total != page.total {
                return Err("道理鱼曲库总数在扫描过程中发生变化，请重试".to_string());
            }
        }
        expected_total = Some(page.total);

        if page.total == 0 {
            if skip != 0 || page.raw_count != 0 {
                return Err("道理鱼分页与曲库总数不一致".to_string());
            }
            break;
        }
        if page.raw_count == 0 || skip > page.total.saturating_sub(page.raw_count as u64) {
            return Err("道理鱼分页出现空页或越过曲库末尾".to_string());
        }

        for track in &page.tracks {
            if !seen.insert(track.id.clone()) {
                return Err(format!("道理鱼曲库出现重复曲目: {}", track.id));
            }
            // 格式不可识别的曲目跳过而不是报错——那是解码能力差异，不是协议违背。
            if let Some(song) = track.to_scanned_song(config, cover_cache) {
                songs.push(song);
            } else {
                log::debug!("[道理鱼] 跳过无可用格式的曲目: {} ({:?})", track.title, track.extension);
            }
        }

        skip += page.raw_count as u64;
        if skip >= page.total {
            break;
        }
        if page.raw_count != PAGE_SIZE {
            return Err("道理鱼分页未按预期满页返回".to_string());
        }

        if let Some(tx) = &progress {
            let _ = tx.send((songs.len(), page.total as usize));
        }

        if page_index + 1 == MAX_PAGES {
            return Err("道理鱼曲库分页超过安全上限".to_string());
        }
    }

    if let Some(tx) = &progress {
        let total = songs.len();
        let _ = tx.send((total, total));
    }
    Ok(songs)
}

/// 连接测试：登录 + 拉一页（`take=1`）读总数。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    if api_url(config).is_none() {
        return ConnectionTestResult {
            success: false,
            message: "未填写道理鱼服务器地址".to_string(),
            server_version: None,
        };
    }
    match fetch_page(config, 0, 1) {
        Ok(page) => ConnectionTestResult {
            success: true,
            message: format!("已连接到道理鱼（曲库 {} 首）", page.total),
            server_version: None,
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

// ==================== 播放 ====================

/// 播放地址（原始字节、支持 Range；`/stream` 会转码，故不用）。
fn download_url(config: &StreamServerConfig, id: &str) -> Option<String> {
    let encoded: String = url::form_urlencoded::byte_serialize(id.as_bytes()).collect();
    Some(format!("{}/tracks/{encoded}/download", api_url(config)?))
}

/// 生成播放 URL（经本地桥接，认证头由桥接在每次请求前补上）。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    match bridge::register(config, song_id) {
        Ok(url) => url,
        Err(e) => {
            log::error!("[道理鱼] 建立本地桥接失败: {e}");
            String::new()
        }
    }
}

/// 构造可随机读取的桥接读取器。
pub fn open_reader(
    config: &StreamServerConfig,
    song_id: &str,
) -> Result<Box<dyn RangeReader>, String> {
    let id = track_id(song_id).ok_or_else(|| format!("无法识别的道理鱼曲目引用: {song_id}"))?;
    let url = download_url(config, &id).ok_or("未填写道理鱼服务器地址")?;

    let client = build_blocking_client_with_timeout(
        &config.server_url,
        BRIDGE_CONNECT_TIMEOUT,
        BRIDGE_REQUEST_TIMEOUT,
    )?;

    let owned = config.clone();
    let headers: Arc<HttpHeaderProvider> = Arc::new(move || bearer_headers(&owned));

    let total = bridge::probe_total_len(&client, &url, headers.as_ref())?;
    let opener = bridge::http_segment_opener(client, url, headers);
    Ok(Box::new(SpoolReader::new(total, opener)?))
}

/// 歌词：拉曲目详情取同步歌词优先。
pub fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let id = track_id(song_id)?;
    let encoded: String = url::form_urlencoded::byte_serialize(id.as_bytes()).collect();
    let url = format!("{}/tracks/{encoded}", api_url(config)?);
    let value = authorized_get(config, &url).ok()?;
    // 详情可能包一层 `{track: {...}}`。
    let track = value.get("track").filter(|v| v.is_object()).unwrap_or(&value);
    let parsed = Track::from_json(track)?;
    parsed.lyrics_sync.or(parsed.lyrics_plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> StreamServerConfig {
        StreamServerConfig {
            server_type: crate::models::ServerType::DaoLiYu,
            server_name: "dl".to_string(),
            server_url: "https://music.example.com".to_string(),
            username: "qa@example.com".to_string(),
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
        let path = track_path("trk_123", "FLAC");
        assert_eq!(path, "/daoliyu/tracks/trk_123.flac");
        assert_eq!(track_id(&path).as_deref(), Some("trk_123"));
        assert_eq!(track_id("/daoliyu/tracks/abc.bin").as_deref(), Some("abc"));
        assert_eq!(track_id("/other/tracks/x.flac"), None);
        assert_eq!(track_id("/daoliyu/tracks/"), None);
    }

    #[test]
    fn base_path_and_api_prefix_are_not_duplicated() {
        let mut cfg = config();
        assert_eq!(api_url(&cfg).as_deref(), Some("https://music.example.com/api"));

        cfg.base_path = Some("/music/".to_string());
        assert_eq!(
            api_url(&cfg).as_deref(),
            Some("https://music.example.com/music/api")
        );

        // 用户把 /api 一起填进地址时不应拼成 /api/api。
        cfg.base_path = None;
        cfg.server_url = "http://192.168.1.9:4000/api".to_string();
        assert_eq!(api_url(&cfg).as_deref(), Some("http://192.168.1.9:4000/api"));
    }

    #[test]
    fn cover_url_handles_absolute_api_and_relative_references() {
        let cfg = config();
        assert_eq!(
            cover_url(&cfg, "https://cdn.example.com/a.jpg").as_deref(),
            Some("https://cdn.example.com/a.jpg")
        );
        assert_eq!(
            cover_url(&cfg, "/api/covers/1").as_deref(),
            Some("https://music.example.com/api/covers/1")
        );
        assert_eq!(
            cover_url(&cfg, "/data/music/cover art.jpg").as_deref(),
            Some("https://music.example.com/api/cover?path=%2Fdata%2Fmusic%2Fcover+art.jpg")
        );
        assert_eq!(cover_url(&cfg, "   "), None);
    }

    #[test]
    fn parses_vnext_track_payload_with_aliases() {
        let json = serde_json::json!({
            "id": "trk_123",
            "title": "Connector Tone",
            "artist": { "id": "artist_1", "name": "Primuse QA" },
            "album": { "id": "album_1", "title": "Validation", "albumArtist": "Various Artists" },
            "filePath": "/data/music/tone.flac",
            "fileSize": 114514,
            "fileFormat": "flac",
            "durationSeconds": 8.0,
            "bitrate": 115,
            "bitDepth": 24,
            "sampleRate": 96000,
            "channels": 2,
            "genres": [{ "id": "g1", "name": "Test" }],
            "createdAt": "2026-08-02T00:00:00.000Z",
            "updatedAt": "2026-08-02T00:01:00.000Z",
            "coverArtPath": "/data/music/cover.jpg",
            "lyricsSync": "[00:00.00]Connector"
        });
        let track = Track::from_json(&json).expect("应能解析");
        assert_eq!(track.id, "trk_123");
        assert_eq!(track.title, "Connector Tone");
        assert_eq!(track.artist, "Primuse QA");
        assert_eq!(track.album, "Validation");
        assert_eq!(track.album_artist, "Various Artists");
        assert_eq!(track.extension.as_deref(), Some("flac"));
        assert_eq!(track.duration, 8.0);
        assert_eq!(track.bitrate, Some(115));
        assert_eq!(track.bit_depth, Some(24));
        assert_eq!(track.channels, Some(2));
        assert_eq!(track.lyrics_sync.as_deref(), Some("[00:00.00]Connector"));
        assert!(track.created_at.is_some());

        let song = track.to_scanned_song(&config(), None).expect("应能转 song");
        assert_eq!(song.id, "/daoliyu/tracks/trk_123.flac");
        assert_eq!(song.duration, 8.0);
        assert_eq!(song.is_sq, Some(true));
        assert_eq!(song.is_hr, Some(true));
        assert_eq!(song.format.as_deref(), Some("FLAC"));
        assert_eq!(
            song.cover_url.as_deref(),
            Some("https://music.example.com/api/cover?path=%2Fdata%2Fmusic%2Fcover.jpg")
        );
    }

    #[test]
    fn duration_and_timestamp_accept_strings_and_milliseconds() {
        let json = serde_json::json!({
            "id": "t1",
            "title": "x",
            "duration": "12.5",
            "fileFormat": "mp3",
            "createdAt": 1_700_000_000_000i64,
        });
        let track = Track::from_json(&json).expect("应能解析");
        assert_eq!(track.duration, 12.5);
        assert_eq!(track.created_at, Some(1_700_000_000));
        assert_eq!(track.extension.as_deref(), Some("mp3"));
    }

    #[test]
    fn unrecognized_format_is_rejected_rather_than_registered() {
        let json = serde_json::json!({ "id": "t2", "title": "x", "fileFormat": "exe" });
        let track = Track::from_json(&json).expect("元数据仍应解析");
        assert_eq!(track.extension, None);
        assert!(track.to_scanned_song(&config(), None).is_none());
    }

    #[test]
    fn page_parsing_validates_shape() {
        let ok = serde_json::json!({
            "items": [{"id": "a", "fileFormat": "flac"}, {"id": "b", "fileFormat": "mp3"}],
            "total": 2, "skip": 0, "take": 2
        });
        let page = parse_page(&ok).expect("应能解析");
        assert_eq!(page.total, 2);
        assert_eq!(page.raw_count, 2);
        assert_eq!(page.tracks.len(), 2);

        assert!(parse_page(&serde_json::json!({ "total": 1 })).is_err());
        assert!(parse_page(&serde_json::json!({ "items": [], "total": -1 })).is_err());
    }

    #[test]
    fn lyrics_accept_string_and_object_forms() {
        assert_eq!(
            lyrics_text(Some(&serde_json::json!("[00:01]hi"))).as_deref(),
            Some("[00:01]hi")
        );
        assert_eq!(
            lyrics_text(Some(&serde_json::json!({"content": "[00:01]hi"}))).as_deref(),
            Some("[00:01]hi")
        );
        assert_eq!(lyrics_text(Some(&serde_json::json!({"nope": 1}))), None);
        assert_eq!(lyrics_text(None), None);
    }

    #[test]
    fn download_url_uses_original_bytes_route() {
        let url = download_url(&config(), "trk_123").expect("应生成");
        assert_eq!(url, "https://music.example.com/api/tracks/trk_123/download");
        assert!(!url.ends_with("/stream"));
    }
}
