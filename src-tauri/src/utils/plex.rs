//! Plex 媒体服务器音乐源（阶段 C1）。
//!
//! 为什么 Plex 属于「低成本」接入：
//! - 曲目资源是 **HTTP 直链**（`<Part key="/library/parts/…/file.flac">`），拼上
//!   令牌即可播放，直接复用播放引擎 + A5 Range 写透 + A7 受信主机放行；
//! - 形态与 Jellyfin/Emby 同族（媒体服务器：库扫描 + 只读），没有「目录浏览」
//!   语义，因此不声明 `BROWSE`。
//!
//! **认证与 Jellyfin 的关键差别**：Plex 不用 `Bearer`，而是 `X-Plex-Token`
//! 请求头。令牌来源有两条路：
//! 1. 用户从 Plex 网页端复制后直接粘贴（`access_token`，优先）；
//! 2. 用 Plex 账号到 `plex.tv/users/sign_in.xml` 换取（`username`/`password`）。
//!
//! Plex 默认返回 XML，本模块沿用项目已有的 `roxmltree`（与 S3/UPnP 一致），
//! 不引入新的依赖，也避开不同 Plex 版本 JSON 字段的漂移。
//!
//! `StreamServerConfig` 字段映射：
//! | 字段 | 含义 |
//! |---|---|
//! | `server_url` | Plex 服务器地址，如 `http://192.168.1.10:32400` |
//! | `access_token` | Plex 令牌（存在则直接使用，不再走账号换取） |
//! | `username` / `password` | 可选：plex.tv 账号，用于换取令牌 |
//! | `base_path` | 可选：只扫某个音乐库（section key，如 `3`）；缺省扫全部音乐库 |
//! | `user_id` | 认证返回的用户 uuid（仅记录） |

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::audio::extract_filename_from_path_str;
use crate::utils::http::{build_blocking_client_for, build_client_for};

/// 无损容器（与 Jellyfin 模块保持同一判定口径）。
const LOSSLESS_CONTAINERS: &[&str] = &["flac", "wav", "ape", "aiff", "dsf", "dff", "alac"];

/// Plex 要求客户端带一个**稳定**的标识（同一标识换取的令牌才长期有效）。
const CLIENT_IDENTIFIER: &str = "bayin-desktop";
const PRODUCT: &str = "BaYin";
const VERSION: &str = "1.0.0";

/// 分页拉取库内容时每页条数。
const PAGE_SIZE: u32 = 200;
/// 安全上限：防止异常响应导致无限翻页。
const MAX_PAGES: usize = 5000;

// ==================== 基础工具 ====================

fn base_url(config: &StreamServerConfig) -> String {
    config.server_url.trim().trim_end_matches('/').to_string()
}

fn token_of(config: &StreamServerConfig) -> Option<&str> {
    config
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Plex 通用请求头。
///
/// `X-Plex-Token` **只在有令牌时**加入——没有令牌时仍然要能发出
/// `/library/sections` 之类的请求去拿到 401，从而给出可读的报错。
fn plex_headers(config: &StreamServerConfig) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("X-Plex-Product", PRODUCT.to_string()),
        ("X-Plex-Version", VERSION.to_string()),
        ("X-Plex-Client-Identifier", CLIENT_IDENTIFIER.to_string()),
        ("X-Plex-Platform", "Desktop".to_string()),
        ("Accept", "application/xml".to_string()),
    ];
    if let Some(token) = token_of(config) {
        headers.push(("X-Plex-Token", token.to_string()));
    }
    headers
}

fn apply_headers(
    mut req: reqwest::blocking::RequestBuilder,
    config: &StreamServerConfig,
) -> reqwest::blocking::RequestBuilder {
    for (k, v) in plex_headers(config) {
        req = req.header(k, v);
    }
    req
}

/// 带认证头发起 GET 并返回响应体文本。
fn get_text(config: &StreamServerConfig, url: &str) -> Result<String, String> {
    let client = build_blocking_client_for(url)?;
    let req = apply_headers(client.get(url), config);
    let resp = req.send().map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        // Plex 在 401 时返回的是空体，把状态码翻译成人话更有用。
        if status.as_u16() == 401 {
            return Err("Plex 认证失败（401）：请检查 X-Plex-Token 或账号密码".to_string());
        }
        return Err(format!("Plex 请求失败: HTTP {}", status.as_u16()));
    }
    Ok(body)
}

/// 解析 XML 根元素属性（Plex 把元数据几乎全部放在属性上）。
fn parse_doc(xml: &str) -> Result<roxmltree::Document<'_>, String> {
    roxmltree::Document::parse(xml).map_err(|e| format!("解析 Plex XML 失败: {e}"))
}

fn attr_u64(node: &roxmltree::Node, name: &str) -> Option<u64> {
    node.attribute(name).and_then(|v| v.trim().parse::<u64>().ok())
}

fn attr_u32(node: &roxmltree::Node, name: &str) -> Option<u32> {
    node.attribute(name).and_then(|v| v.trim().parse::<u32>().ok())
}

fn attr_u8(node: &roxmltree::Node, name: &str) -> Option<u8> {
    node.attribute(name)
        .and_then(|v| v.trim().parse::<u8>().ok())
}

// ==================== 认证 ====================

/// 获取可用令牌。
///
/// 已有 `access_token` 时直接沿用；否则用账号到 plex.tv 换取。
/// 返回 `(token, user_id)`，与 Jellyfin 的 `authenticate` 签名保持一致。
pub async fn authenticate(config: &StreamServerConfig) -> Result<(String, String), String> {
    if let Some(token) = token_of(config) {
        return Ok((
            token.to_string(),
            config.user_id.clone().unwrap_or_default(),
        ));
    }
    if config.username.trim().is_empty() {
        return Err("请填写 Plex 令牌，或填写 Plex 账号以自动获取令牌".to_string());
    }

    let client = build_client_for("https://plex.tv")?;
    let mut req = client
        .post("https://plex.tv/users/sign_in.xml")
        .basic_auth(config.username.trim(), Some(config.password.as_str()));
    for (k, v) in plex_headers(config) {
        req = req.header(k, v);
    }

    let resp = req.send().await.map_err(|e| format!("连接 plex.tv 失败: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Plex 账号认证失败: HTTP {}（请检查账号密码）",
            status.as_u16()
        ));
    }

    parse_sign_in(&body)
}

/// 从 `plex.tv/users/sign_in.xml` 响应中取出令牌与用户 uuid。
///
/// 纯函数，便于用真实响应样本离线单测。
pub fn parse_sign_in(xml: &str) -> Result<(String, String), String> {
    let doc = parse_doc(xml)?;
    // 响应根是 `<user>`（不是 MediaContainer）。
    let user = doc
        .descendants()
        .find(|n| n.has_tag_name("user"))
        .ok_or("Plex 认证响应缺少 <user> 元素")?;

    let token = user
        .children()
        .find(|n| n.has_tag_name("authentication-token"))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or("Plex 认证响应缺少 authentication-token（账号或密码可能有误）")?;

    let uuid = user
        .children()
        .find(|n| n.has_tag_name("uuid"))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    Ok((token.to_string(), uuid))
}

// ==================== 音乐库与曲目解析 ====================

/// Plex 音乐库分区。
#[derive(Debug, Clone, PartialEq)]
pub struct PlexSection {
    pub key: String,
    pub title: String,
}

/// 从 `/library/sections` 响应中挑出音乐库。
///
/// **必须过滤 `type="artist"`**：同一个 Plex 上通常还挂着电影/剧集库，
/// 不加过滤会把整个视频库当成音乐库去扫。
pub fn parse_sections(xml: &str) -> Result<Vec<PlexSection>, String> {
    let doc = parse_doc(xml)?;
    let mut out = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("Directory")) {
        if node.attribute("type") != Some("artist") {
            continue;
        }
        let key = match node.attribute("key") {
            Some(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => continue,
        };
        out.push(PlexSection {
            key,
            title: node.attribute("title").unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

/// 从库曲目响应中解析出的曲目原始信息。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlexTrack {
    pub rating_key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub container: Option<String>,
    pub bitrate: Option<u32>,
    pub channels: Option<u8>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    /// `<Part key>`：形如 `/library/parts/123/1700000000/file.flac`，播放直链的路径部分。
    pub part_key: String,
    pub part_size: u64,
    pub file_path: String,
    pub thumb: Option<String>,
    pub added_at: Option<i64>,
    pub updated_at: Option<i64>,
}

/// 解析曲目页，返回 `(曲目, totalSize)`。
///
/// `totalSize` 缺省时回退为 `size`，避免调用方误判为"还有下一页"而无限翻页。
pub fn parse_tracks(xml: &str) -> Result<(Vec<PlexTrack>, u32), String> {
    let doc = parse_doc(xml)?;
    let container = doc
        .descendants()
        .find(|n| n.has_tag_name("MediaContainer"))
        .ok_or("Plex 响应缺少 <MediaContainer>")?;

    let total = attr_u32(&container, "totalSize")
        .or_else(|| attr_u32(&container, "size"))
        .unwrap_or(0);

    let tracks = doc
        .descendants()
        .filter(|n| n.has_tag_name("Track"))
        .map(parse_track_node)
        .collect();

    Ok((tracks, total))
}

fn parse_track_node(node: roxmltree::Node) -> PlexTrack {
    let media = node.children().find(|n| n.has_tag_name("Media"));
    let part = media
        .as_ref()
        .and_then(|m| m.children().find(|n| n.has_tag_name("Part")));
    // 音频流（streamType=2）；Plex 可能同时挂字幕/视频流。
    let stream = part
        .as_ref()
        .and_then(|p| {
            p.children()
                .find(|n| n.has_tag_name("Stream") && n.attribute("streamType") == Some("2"))
        })
        .or_else(|| {
            media.as_ref().and_then(|m| {
                m.children()
                    .find(|n| n.has_tag_name("Stream") && n.attribute("streamType") == Some("2"))
            })
        });

    let file_path = part
        .as_ref()
        .and_then(|p| p.attribute("file"))
        .unwrap_or_default()
        .to_string();

    let title = match node.attribute("title") {
        Some(t) if !t.trim().is_empty() => t.to_string(),
        _ => extract_filename_from_path_str(&file_path)
            .unwrap_or_else(|| node.attribute("ratingKey").unwrap_or_default().to_string()),
    };

    PlexTrack {
        rating_key: node
            .attribute("ratingKey")
            .or_else(|| node.attribute("key"))
            .unwrap_or_default()
            .to_string(),
        title,
        artist: node
            .attribute("grandparentTitle")
            .or_else(|| node.attribute("originalTitle"))
            .unwrap_or("未知艺术家")
            .to_string(),
        album: node
            .attribute("parentTitle")
            .unwrap_or("未知专辑")
            .to_string(),
        duration_ms: attr_u64(&node, "duration").unwrap_or(0),
        container: media
            .as_ref()
            .and_then(|m| m.attribute("container"))
            .or_else(|| part.as_ref().and_then(|p| p.attribute("container")))
            .map(|s| s.to_ascii_lowercase()),
        bitrate: media.as_ref().and_then(|m| attr_u32(m, "bitrate")),
        channels: media
            .as_ref()
            .and_then(|m| attr_u8(m, "audioChannels"))
            .or_else(|| stream.as_ref().and_then(|s| attr_u8(s, "channels"))),
        sample_rate: stream.as_ref().and_then(|s| attr_u32(s, "samplingRate")),
        bit_depth: stream.as_ref().and_then(|s| attr_u8(s, "bitDepth")),
        part_key: part
            .as_ref()
            .and_then(|p| p.attribute("key"))
            .unwrap_or_default()
            .to_string(),
        part_size: part.as_ref().and_then(|p| attr_u64(p, "size")).unwrap_or(0),
        file_path,
        thumb: node.attribute("thumb").map(|s| s.to_string()),
        added_at: node.attribute("addedAt").and_then(|v| v.trim().parse().ok()),
        updated_at: node.attribute("updatedAt").and_then(|v| v.trim().parse().ok()),
    }
}

/// 把 Plex 曲目转成项目通用的 `ScannedSong`。
pub fn to_scanned_song(track: &PlexTrack, server_url: &str, token: Option<&str>) -> ScannedSong {
    let container = track.container.clone().unwrap_or_default();
    let is_sq = LOSSLESS_CONTAINERS.contains(&container.as_str());
    let is_hr = track.bit_depth.map(|d| d > 16).unwrap_or(false)
        || track.sample_rate.map(|r| r > 44100).unwrap_or(false);

    // 封面走 `<img>` 直接加载，无法附加请求头，因此令牌必须落在 URL 上
    // （与 Jellyfin 的 `?api_key=` 同一处理方式）。
    let cover_url = track.thumb.as_ref().and_then(|thumb| {
        let token = token?;
        if thumb.starts_with("http") {
            Some(format!("{thumb}?X-Plex-Token={token}"))
        } else {
            Some(format!(
                "{}{}?X-Plex-Token={}",
                server_url.trim_end_matches('/'),
                thumb,
                token
            ))
        }
    });

    ScannedSong {
        id: track.rating_key.clone(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration: track.duration_ms as f64 / 1000.0,
        file_path: track.file_path.clone(),
        file_size: track.part_size,
        cover_url,
        cover_hash: None,
        file_modified: track.updated_at.or(track.added_at),
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
        format: if container.is_empty() {
            None
        } else {
            Some(container.to_uppercase())
        },
        bit_depth: track.bit_depth,
        sample_rate: track.sample_rate,
        bitrate: track.bitrate,
        channels: track.channels,
        created_at: track.added_at,
    }
}

// ==================== 网络调用 ====================

/// 探测服务器版本。
///
/// `/identity` 是 Plex 官方给出的**免认证**探活端点；万一旧版本没有，
/// 退回到根路径再试一次。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    let base = base_url(config);
    if base.is_empty() {
        return ConnectionTestResult {
            success: false,
            message: "未填写 Plex 服务器地址".to_string(),
            server_version: None,
        };
    }

    let mut last_error = String::new();
    for path in ["/identity", "/"] {
        match get_text(config, &format!("{base}{path}")) {
            Ok(body) => {
                let version = parse_doc(&body)
                    .ok()
                    .and_then(|doc| {
                        doc.descendants()
                            .find(|n| n.has_tag_name("MediaContainer"))
                            .and_then(|n| n.attribute("version"))
                            .map(|s| s.to_string())
                    });
                let name = if version.is_some() { "Plex" } else { "Plex（未知版本）" };
                return ConnectionTestResult {
                    success: true,
                    message: format!("已连接到 {name}"),
                    server_version: version,
                };
            }
            Err(e) => last_error = e,
        }
    }

    ConnectionTestResult {
        success: false,
        message: last_error,
        server_version: None,
    }
}

/// 列出音乐库（`base_path` 指定时只返回它）。
pub fn music_sections(config: &StreamServerConfig) -> Result<Vec<PlexSection>, String> {
    let base = base_url(config);
    let body = get_text(config, &format!("{base}/library/sections"))?;
    let mut sections = parse_sections(&body)?;

    if let Some(filter) = config
        .base_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        sections.retain(|s| s.key == filter);
    }
    Ok(sections)
}

/// 拉取单个音乐库的全部曲目（自动翻页）。
pub fn fetch_section_tracks(
    config: &StreamServerConfig,
    section_key: &str,
) -> Result<Vec<PlexTrack>, String> {
    let base = base_url(config);
    let mut out = Vec::new();
    let mut start: u32 = 0;

    for _ in 0..MAX_PAGES {
        let url = format!(
            "{base}/library/sections/{section_key}/all?type=10&sort=titleSort:asc\
             &X-Plex-Container-Start={start}&X-Plex-Container-Size={PAGE_SIZE}"
        );
        let body = get_text(config, &url)?;
        let (page, total) = parse_tracks(&body)?;
        let n = page.len() as u32;
        out.extend(page);

        start += n;
        // 空页 / 已取满 / 不足一页 都视为结束。
        if n == 0 || start >= total || n < PAGE_SIZE {
            break;
        }
    }

    Ok(out)
}

/// 扫描全部音乐库。
pub fn fetch_all_songs(config: &StreamServerConfig) -> Result<Vec<ScannedSong>, String> {
    let token = token_of(config).ok_or("缺少 Plex 令牌，请先认证或粘贴令牌")?;
    let base = base_url(config);
    let sections = music_sections(config)?;
    if sections.is_empty() {
        return Err("该 Plex 服务器上没有音乐库（未找到 type=artist 的分区）".to_string());
    }

    let mut songs = Vec::new();
    for section in &sections {
        for track in fetch_section_tracks(config, &section.key)? {
            songs.push(to_scanned_song(&track, &base, Some(token)));
        }
    }
    Ok(songs)
}

/// 生成播放直链。
///
/// Plex 的 `<Part key>` 本身就是 HTTP 路径，服务器原生支持 Range，因此这是
/// **HTTP 原生源**——无需本地桥接。
///
/// 令牌放进查询串（与 Jellyfin 的 `?api_key=` 一致）：这样无论播放链路
/// 走 `get_stream_url` 还是 `get_stream_play_info`，都能直接播放。
pub fn get_stream_url(config: &StreamServerConfig, song_id: &str) -> String {
    if song_id.trim().is_empty() {
        return String::new();
    }
    let base = base_url(config);
    let path = if song_id.starts_with("http://") || song_id.starts_with("https://") {
        song_id.to_string()
    } else {
        format!("{base}{}", if song_id.starts_with('/') { song_id.to_string() } else { format!("/{song_id}") })
    };
    match token_of(config) {
        Some(token) => {
            let sep = if path.contains('?') { '&' } else { '?' };
            format!("{path}{sep}X-Plex-Token={token}")
        }
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ServerType;

    fn config() -> StreamServerConfig {
        StreamServerConfig {
            server_type: ServerType::Plex,
            server_name: "plex".to_string(),
            server_url: "http://192.168.1.10:32400/".to_string(),
            username: "plexuser".to_string(),
            password: "secret".to_string(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: Some("TOKEN123".to_string()),
            user_id: None,
        }
    }

    const SECTIONS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MediaContainer size="3" allowCameraUpload="0">
  <Directory allowSync="1" key="1" type="movie" title="电影" />
  <Directory allowSync="1" key="2" type="show" title="剧集" />
  <Directory allowSync="1" key="3" type="artist" title="音乐 &amp; 更多" />
</MediaContainer>"#;

    const TRACKS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MediaContainer size="2" totalSize="2" offset="0">
  <Track ratingKey="101" key="/library/metadata/101" type="track"
         title="夜航" grandparentTitle="某人" parentTitle="第一张"
         duration="215000" addedAt="1700000000" updatedAt="1700000100"
         thumb="/library/metadata/201/thumb/1700000100">
    <Media id="1" duration="215000" bitrate="1411" audioChannels="2" audioCodec="flac" container="flac">
      <Part id="9" key="/library/parts/9/1700000100/file.flac"
            file="/music/某人/第一张/夜航.flac" size="30000000" container="flac">
        <Stream id="5" streamType="2" codec="flac" bitDepth="24" samplingRate="96000" channels="2" />
      </Part>
    </Media>
  </Track>
  <Track ratingKey="102" key="/library/metadata/102" type="track"
         title="No Album" grandparentTitle="Someone" duration="180000">
    <Media id="2" bitrate="320" audioChannels="2" audioCodec="mp3" container="mp3">
      <Part id="10" key="/library/parts/10/1700000200/track.mp3"
            file="/music/a/track.mp3" size="4000000" container="mp3">
        <Stream id="6" streamType="2" codec="mp3" bitDepth="16" samplingRate="44100" channels="2" />
      </Part>
    </Media>
  </Track>
</MediaContainer>"#;

    // ---------- 分区解析 ----------

    /// 电影/剧集库必须被过滤掉，否则会把整个视频库当音乐扫。
    #[test]
    fn sections_only_keep_music_libraries() {
        let sections = parse_sections(SECTIONS_XML).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].key, "3");
        // XML 实体应被解码。
        assert_eq!(sections[0].title, "音乐 & 更多");
    }

    #[test]
    fn sections_tolerate_empty_and_broken_xml() {
        let empty = r#"<MediaContainer size="0"></MediaContainer>"#;
        assert!(parse_sections(empty).unwrap().is_empty());
        assert!(parse_sections("not xml at all").is_err());
        // 缺 key 的 Directory 直接跳过，不产生半条记录。
        let no_key = r#"<MediaContainer><Directory type="artist" title="x" /></MediaContainer>"#;
        assert!(parse_sections(no_key).unwrap().is_empty());
    }

    // ---------- 曲目解析 ----------

    #[test]
    fn parses_track_attributes_and_media_quality() {
        let (tracks, total) = parse_tracks(TRACKS_XML).unwrap();
        assert_eq!(total, 2);
        assert_eq!(tracks.len(), 2);

        let t = &tracks[0];
        assert_eq!(t.rating_key, "101");
        assert_eq!(t.title, "夜航");
        assert_eq!(t.artist, "某人");
        assert_eq!(t.album, "第一张");
        assert_eq!(t.duration_ms, 215_000);
        assert_eq!(t.container.as_deref(), Some("flac"));
        assert_eq!(t.bitrate, Some(1411));
        assert_eq!(t.channels, Some(2));
        assert_eq!(t.sample_rate, Some(96000));
        assert_eq!(t.bit_depth, Some(24));
        assert_eq!(t.part_key, "/library/parts/9/1700000100/file.flac");
        assert_eq!(t.part_size, 30_000_000);
        assert_eq!(t.file_path, "/music/某人/第一张/夜航.flac");
        assert_eq!(t.updated_at, Some(1_700_000_100));
    }

    /// 缺 album / thumb / 无损流时不能 panic，且要有合理兜底值。
    #[test]
    fn parses_track_with_missing_optional_fields() {
        let (tracks, _) = parse_tracks(TRACKS_XML).unwrap();
        let t = &tracks[1];
        assert_eq!(t.album, "未知专辑");
        assert_eq!(t.thumb, None);
        assert_eq!(t.added_at, None);
        assert_eq!(t.updated_at, None);
    }

    #[test]
    fn track_without_title_falls_back_to_filename() {
        let xml = r#"<MediaContainer size="1" totalSize="1">
          <Track ratingKey="7" duration="1000">
            <Media container="mp3"><Part key="/p" file="/music/x/未命名.mp3" /></Media>
          </Track>
        </MediaContainer>"#;
        let (tracks, _) = parse_tracks(xml).unwrap();
        assert_eq!(tracks[0].title, "未命名");
    }

    #[test]
    fn tracks_from_empty_page_are_empty() {
        let xml = r#"<MediaContainer size="0" totalSize="0" />"#;
        let (tracks, total) = parse_tracks(xml).unwrap();
        assert!(tracks.is_empty());
        assert_eq!(total, 0);
    }

    // ---------- ScannedSong 转换 ----------

    #[test]
    fn converts_track_to_scanned_song_with_cover_token() {
        let (tracks, _) = parse_tracks(TRACKS_XML).unwrap();
        let song = to_scanned_song(&tracks[0], "http://192.168.1.10:32400", Some("TOKEN123"));

        assert_eq!(song.id, "101");
        assert_eq!(song.duration, 215.0);
        assert_eq!(song.file_size, 30_000_000);
        assert_eq!(song.is_sq, Some(true));
        assert_eq!(song.is_hr, Some(true), "24bit/96kHz 应判为 Hi-Res");
        assert_eq!(song.format.as_deref(), Some("FLAC"));
        assert_eq!(
            song.cover_url.as_deref(),
            Some("http://192.168.1.10:32400/library/metadata/201/thumb/1700000100?X-Plex-Token=TOKEN123")
        );
    }

    /// 无令牌时不生成带 `X-Plex-Token=` 的封面 URL（避免出现坏链接）。
    #[test]
    fn cover_url_requires_token() {
        let (tracks, _) = parse_tracks(TRACKS_XML).unwrap();
        let song = to_scanned_song(&tracks[0], "http://h:32400", None);
        assert!(song.cover_url.is_none());
    }

    /// 16bit/44.1kHz 的 mp3 既不是无损也不是 Hi-Res。
    #[test]
    fn mp3_is_neither_lossless_nor_hires() {
        let (tracks, _) = parse_tracks(TRACKS_XML).unwrap();
        let song = to_scanned_song(&tracks[1], "http://h:32400", Some("T"));
        assert_eq!(song.is_sq, Some(false));
        assert_eq!(song.is_hr, Some(false));
        assert_eq!(song.format.as_deref(), Some("MP3"));
    }

    // ---------- 播放 URL ----------

    #[test]
    fn stream_url_is_http_native_with_token() {
        let c = config();
        let url = get_stream_url(&c, "/library/parts/9/1700000100/file.flac");
        assert_eq!(
            url,
            "http://192.168.1.10:32400/library/parts/9/1700000100/file.flac?X-Plex-Token=TOKEN123"
        );
        // 末段保留扩展名，供解码器给出 symphonia 探测提示。
        assert!(url.contains("file.flac?"), "{url}");
    }

    #[test]
    fn stream_url_handles_missing_slash_and_existing_query() {
        let c = config();
        assert_eq!(
            get_stream_url(&c, "library/parts/1/a.mp3"),
            "http://192.168.1.10:32400/library/parts/1/a.mp3?X-Plex-Token=TOKEN123"
        );
        assert_eq!(
            get_stream_url(&c, "http://other:32400/p?a=1"),
            "http://other:32400/p?a=1&X-Plex-Token=TOKEN123"
        );
        // 空 song_id 不应产出"服务器根 + token"这种误导性 URL。
        assert_eq!(get_stream_url(&c, "  "), "");
    }

    /// 没有令牌时仍返回可用路径（播放会 401，但错误位置明确，便于排查）。
    #[test]
    fn stream_url_without_token_has_no_query() {
        let mut c = config();
        c.access_token = None;
        assert_eq!(
            get_stream_url(&c, "/library/parts/1/a.mp3"),
            "http://192.168.1.10:32400/library/parts/1/a.mp3"
        );
    }

    // ---------- 认证 ----------

    #[test]
    fn plex_headers_carry_token_only_when_present() {
        let c = config();
        let h = plex_headers(&c);
        assert!(h.iter().any(|(k, v)| *k == "X-Plex-Token" && v == "TOKEN123"));
        // Plex 客户端标识必须始终存在。
        assert!(h.iter().any(|(k, _)| *k == "X-Plex-Client-Identifier"));

        let mut anon = config();
        anon.access_token = None;
        assert!(!plex_headers(&anon).iter().any(|(k, _)| *k == "X-Plex-Token"));
    }

    #[test]
    fn sign_in_response_yields_token_and_uuid() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<user id="1" title="plexuser">
  <uuid>abcdef012345</uuid>
  <authentication-token>TOKEN_ABC</authentication-token>
</user>"#;
        let (token, uuid) = parse_sign_in(xml).unwrap();
        assert_eq!(token, "TOKEN_ABC");
        assert_eq!(uuid, "abcdef012345");
    }

    /// 账号密码错误时 plex.tv 返回没有 token 的 `<user>`，必须给出可读错误。
    #[test]
    fn sign_in_without_token_is_an_error() {
        let xml = r#"<user id="1" title="plexuser"><uuid>x</uuid></user>"#;
        let err = parse_sign_in(xml).unwrap_err();
        assert!(err.contains("authentication-token"), "{err}");
        assert!(parse_sign_in("<html>502</html>").is_err());
    }

    /// 已有令牌时认证应短路，不去访问 plex.tv（离线也应立即成功）。
    #[test]
    fn authenticate_short_circuits_with_existing_token() {
        let mut c = config();
        c.user_id = Some("uuid-1".to_string());
        let (token, uuid) = pollster::block_on(authenticate(&c)).unwrap();
        assert_eq!(token, "TOKEN123");
        assert_eq!(uuid, "uuid-1");
    }

    #[test]
    fn authenticate_without_token_or_username_is_an_error() {
        let mut c = config();
        c.access_token = None;
        c.username = String::new();
        assert!(pollster::block_on(authenticate(&c)).is_err());
    }

    // ---------- 测试连接 ----------

    #[test]
    fn test_connection_without_url_fails_fast() {
        let mut c = config();
        c.server_url = "   ".to_string();
        let r = test_connection(&c);
        assert!(!r.success);
        assert!(r.message.contains("未填写"));
    }
}
