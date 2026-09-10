//! S3 / S3 兼容对象存储音乐源（阶段 B5）。
//!
//! 设计要点：
//! - **不引入重量级 SDK**：用 `reqwest` + 手写 AWS SigV4 签名（HMAC-SHA256 基于
//!   已有 `sha2` 实现），依赖面极小、跨平台、签名逻辑可离线单测。
//! - **HTTP 原生**：对象直链即 HTTP(S)，可直接复用现有播放引擎（`HttpStreamSource`）、
//!   A5 Range 稀疏缓存写透与 A7 可信主机连接放行。
//! - 兼容 MinIO / 群晖 / 飞牛等 S3 网关（path-style 寻址）。
//!
//! `StreamServerConfig` 字段映射（复用既有字段，避免加表）：
//! | 字段 | 含义 |
//! |---|---|
//! | `server_url` | Endpoint，如 `https://minio.local:9000` |
//! | `base_path`  | `bucket` 或 `bucket/prefix` |
//! | `username`   | Access Key ID |
//! | `password`   | Secret Access Key |
//! | `user_id`    | Region 覆盖（可空 → 由 Endpoint 推断，默认 `us-east-1`） |
//! | `legacy_auth`| `true` 强制 path-style 寻址 |

use std::collections::HashMap;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::cover::CoverCache;
use crate::utils::http::build_blocking_client_for;
use sha2::{Digest, Sha256};

const SERVICE: &str = "s3";
/// `sha256("")`：GET/HEAD 空载荷的规范哈希。
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
/// 预签名 URL 的载荷哈希占位（SigV4 query 签名规定值）。
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
const DEFAULT_REGION: &str = "us-east-1";
/// 预签名 URL 有效期：6 小时（足够一次完整播放/跳转，避免流中途签名失效）。
const PRESIGN_TTL_SECS: u64 = 6 * 60 * 60;
const METADATA_PROBE_BYTES: u64 = 1024 * 1024;

/// 可被扫描入音乐的扩展名。
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "ape", "wv", "aiff", "aif", "alac", "dsf",
    "dff", "mka",
];

// ---------------------------------------------------------------------------
// 配置
// ---------------------------------------------------------------------------

/// S3 连接参数（已从 `StreamServerConfig` 归一化）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Config {
    /// 形如 `https://host[:port]`（不含尾部斜杠）。
    pub endpoint: String,
    pub region: String,
    /// 桶名（不含斜杠）。
    pub bucket: String,
    /// 对象前缀（不含首尾斜杠；空串表示桶根）。
    pub prefix: String,
    pub access_key: String,
    pub secret_key: String,
    /// `true` = `endpoint/bucket/key`；`false` = `bucket.endpoint/key`。
    pub path_style: bool,
}

impl S3Config {
    /// 从统一配置解析。`base_path` = `bucket` 或 `bucket/prefix`。
    pub fn from_stream_config(config: &StreamServerConfig) -> Result<Self, String> {
        let endpoint = config.server_url.trim().trim_end_matches('/').to_string();
        if endpoint.is_empty() {
            return Err("S3 Endpoint 未配置".to_string());
        }
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err("S3 Endpoint 需以 http:// 或 https:// 开头".to_string());
        }

        let base = config.base_path.as_deref().unwrap_or("").trim_matches('/');
        let mut parts = base.splitn(2, '/');
        let bucket = parts.next().unwrap_or("").trim().to_string();
        let prefix = parts.next().unwrap_or("").trim_matches('/').to_string();
        if bucket.is_empty() {
            return Err("S3 Bucket 未配置（请填写在「基础路径」中）".to_string());
        }

        let region = config
            .user_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| detect_region(&endpoint));

        // path-style：显式开启，或非 AWS 域（自建 MinIO/群晖等默认 path-style）。
        let path_style = config.legacy_auth || !is_aws_endpoint(&endpoint);

        Ok(Self {
            endpoint,
            region,
            bucket,
            prefix,
            access_key: config.username.trim().to_string(),
            secret_key: config.password.trim().to_string(),
            path_style,
        })
    }

    /// 无凭据（公开桶）：跳过签名。
    pub fn is_anonymous(&self) -> bool {
        self.access_key.is_empty() || self.secret_key.is_empty()
    }

    /// 完整对象 URL（未签名）。
    pub fn object_url(&self, key: &str) -> Result<String, String> {
        let ep = parse_endpoint(&self.endpoint)?;
        let encoded = uri_encode(key, false);
        if self.path_style {
            Ok(format!(
                "{}/{}/{}",
                self.endpoint,
                uri_encode(&self.bucket, false),
                encoded
            ))
        } else {
            Ok(format!(
                "{}://{}.{}{}",
                ep.scheme,
                self.bucket,
                ep.host_port(),
                path_of(&encoded)
            ))
        }
    }

    /// 桶级 URL（列举对象用）。
    pub fn bucket_url(&self) -> Result<String, String> {
        let ep = parse_endpoint(&self.endpoint)?;
        if self.path_style {
            Ok(format!("{}/{}", self.endpoint, uri_encode(&self.bucket, false)))
        } else {
            Ok(format!(
                "{}://{}.{}{}",
                ep.scheme,
                self.bucket,
                ep.host_port(),
                "/"
            ))
        }
    }

    /// 把 `bucket` + `prefix` + `key` 组合成对象键。
    pub fn full_key(&self, key: &str) -> String {
        let k = key.trim_start_matches('/');
        if self.prefix.is_empty() {
            k.to_string()
        } else if k.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{}", self.prefix, k)
        }
    }

    /// 去掉 `prefix`，得到展示用的相对键。
    pub fn relative_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            key
                .strip_prefix(&format!("{}/", self.prefix))
                .unwrap_or(key)
                .to_string()
        }
    }
}

/// 解析后的 Endpoint。
struct Endpoint {
    scheme: String,
    host: String,
    port: Option<u16>,
}

impl Endpoint {
    /// `host` 或 `host:port`（非默认端口才带端口）。
    fn host_port(&self) -> String {
        match self.port {
            Some(p) => format!("{}:{}", self.host, p),
            None => self.host.clone(),
        }
    }
}

fn parse_endpoint(endpoint: &str) -> Result<Endpoint, String> {
    let u = url::Url::parse(endpoint).map_err(|e| format!("无效的 S3 Endpoint: {e}"))?;
    let host = u
        .host_str()
        .ok_or_else(|| "S3 Endpoint 缺少主机名".to_string())?
        .to_string();
    Ok(Endpoint {
        scheme: u.scheme().to_string(),
        host,
        port: u.port(),
    })
}

/// 把已编码的键拼成以 `/` 开头的路径。
fn path_of(encoded_key: &str) -> String {
    if encoded_key.is_empty() {
        "/".to_string()
    } else {
        format!("/{encoded_key}")
    }
}

fn is_aws_endpoint(endpoint: &str) -> bool {
    endpoint
        .to_ascii_lowercase()
        .contains("amazonaws.com")
}

/// 从 `s3.<region>.amazonaws.com` / `s3-<region>.amazonaws.com` 等主机名推断 region。
fn detect_region(endpoint: &str) -> String {
    let raw_host = endpoint
        .split("://")
        .nth(1)
        .unwrap_or(endpoint)
        .split('/')
        .next()
        .unwrap_or("");
    let host = raw_host.split(':').next().unwrap_or("").to_ascii_lowercase();

    for marker in [".s3.", "s3.", ".s3-", "s3-"] {
        if let Some(pos) = host.find(marker) {
            let rest = &host[pos + marker.len()..];
            if let Some((region, _)) = rest.split_once(".amazonaws.com") {
                let region = region.trim_matches('.');
                if !region.is_empty() {
                    return region.to_string();
                }
            }
        }
    }
    DEFAULT_REGION.to_string()
}

// ---------------------------------------------------------------------------
// SigV4 签名（纯逻辑，可离线单测）
// ---------------------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// HMAC-SHA256（基于已有 sha2，避免额外依赖）。
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let d = Sha256::digest(key);
        k[..32].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer.finalize());
    out
}

/// SigV4 派生签名密钥：kDate → kRegion → kService → kSigning。
fn signing_key(secret: &str, datestamp: &str, region: &str) -> [u8; 32] {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), datestamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, SERVICE.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

/// AWS SigV4 URI 编码：仅保留 `A-Za-z0-9-_.~`（+ `/`，当 `encode_slash=false`）。
fn uri_encode(s: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else if c == '/' && !encode_slash {
            out.push('/');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// 规范查询串：按键排序、逐项编码。
fn canonical_query(params: &[(String, String)]) -> String {
    let mut items: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (uri_encode(k, true), uri_encode(v, true)))
        .collect();
    items.sort();
    items
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// 计算签名并返回 `Authorization` 头值。
///
/// `extra_headers`：除 host / x-amz-content-sha256 / x-amz-date 外还要纳入签名的头
/// （如 `range`），名称须为小写、值已归一化。
#[allow(clippy::too_many_arguments)]
fn authorization(
    access_key: &str,
    secret_key: &str,
    region: &str,
    amz_date: &str,
    datestamp: &str,
    method: &str,
    canonical_uri: &str,
    canonical_qs: &str,
    host: &str,
    extra_headers: &[(String, String)],
    payload_hash: &str,
) -> String {
    let mut headers: Vec<(String, String)> = vec![("host".to_string(), host.to_string())];
    headers.extend(extra_headers.iter().cloned());
    headers.push(("x-amz-content-sha256".to_string(), payload_hash.to_string()));
    headers.push(("x-amz-date".to_string(), amz_date.to_string()));
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical_headers: String = headers
        .iter()
        .map(|(k, v)| format!("{k}:{}\n", v.trim()))
        .collect();
    let signed_headers = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");

    // 注意：`canonical_headers` 自身以 `\n` 结尾，这里再补一个 `\n` 形成
    // headers 与 signed headers 之间的空行（SigV4 规范要求）。
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n{canonical_qs}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let scope = format!("{datestamp}/{region}/{SERVICE}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = hex(&hmac_sha256(
        &signing_key(secret_key, datestamp, region),
        string_to_sign.as_bytes(),
    ));

    format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    )
}

/// 为一次具体请求生成签名头（含 `Authorization` / `x-amz-date` / `x-amz-content-sha256`）。
fn signed_get_headers(
    cfg: &S3Config,
    url: &str,
    amz_date: &str,
    datestamp: &str,
) -> Result<Vec<(String, String)>, String> {
    let u = url::Url::parse(url).map_err(|e| format!("无效的请求 URL: {e}"))?;
    let host = host_of(&u)?;
    let auth = authorization(
        &cfg.access_key,
        &cfg.secret_key,
        &cfg.region,
        amz_date,
        datestamp,
        "GET",
        u.path(),
        u.query().unwrap_or(""),
        &host,
        &[],
        EMPTY_SHA256,
    );
    Ok(vec![
        ("Authorization".to_string(), auth),
        ("x-amz-date".to_string(), amz_date.to_string()),
        ("x-amz-content-sha256".to_string(), EMPTY_SHA256.to_string()),
    ])
}

fn host_of(u: &url::Url) -> Result<String, String> {
    let host = u
        .host_str()
        .ok_or_else(|| "URL 缺少主机名".to_string())?
        .to_string();
    Ok(match u.port() {
        Some(p) => format!("{host}:{p}"),
        None => host,
    })
}

/// 时间戳对：`(YYYYMMDDTHHMMSSZ, YYYYMMDD)`。
fn amz_dates(now: chrono::DateTime<chrono::Utc>) -> (String, String) {
    (
        now.format("%Y%m%dT%H%M%SZ").to_string(),
        now.format("%Y%m%d").to_string(),
    )
}

// ---------------------------------------------------------------------------
// 预签名 URL（播放用）
// ---------------------------------------------------------------------------

/// 生成预签名 GET URL（匿名桶直接返回直链）。
pub fn presign_get_url(cfg: &S3Config, key: &str) -> Result<String, String> {
    presign_get_url_with(cfg, key, PRESIGN_TTL_SECS, chrono::Utc::now())
}

/// 指定有效期/时间点的预签名（便于单测）。
pub fn presign_get_url_with(
    cfg: &S3Config,
    key: &str,
    expires: u64,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<String, String> {
    let url = cfg.object_url(&cfg.full_key(key))?;
    if cfg.is_anonymous() {
        return Ok(url);
    }
    let u = url::Url::parse(&url).map_err(|e| format!("无效的请求 URL: {e}"))?;
    let host = host_of(&u)?;
    let (amz_date, datestamp) = amz_dates(now);
    let scope = format!("{datestamp}/{}/{SERVICE}/aws4_request", cfg.region);

    let params: Vec<(String, String)> = vec![
        ("X-Amz-Algorithm".into(), "AWS4-HMAC-SHA256".into()),
        (
            "X-Amz-Credential".into(),
            format!("{}/{scope}", cfg.access_key),
        ),
        ("X-Amz-Date".into(), amz_date.clone()),
        ("X-Amz-Expires".into(), expires.to_string()),
        ("X-Amz-SignedHeaders".into(), "host".into()),
    ];
    let qs = canonical_query(&params);

    let canonical_request =
        format!("GET\n{}\n{qs}\nhost:{host}\n\nhost\n{UNSIGNED_PAYLOAD}", u.path());
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = hex(&hmac_sha256(
        &signing_key(&cfg.secret_key, &datestamp, &cfg.region),
        string_to_sign.as_bytes(),
    ));

    Ok(format!(
        "{}://{}{}?{qs}&X-Amz-Signature={signature}",
        u.scheme(),
        host,
        u.path()
    ))
}

// ---------------------------------------------------------------------------
// 对象列举（ListObjectsV2）
// ---------------------------------------------------------------------------

/// 一个对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Object {
    /// 完整对象键（含 prefix）。
    pub key: String,
    pub size: u64,
    pub last_modified: Option<i64>,
}

/// 一页列举结果。
#[derive(Debug, Clone, Default)]
pub struct ListPage {
    pub objects: Vec<S3Object>,
    /// 以 `delimiter` 聚合的「子目录」前缀。
    pub common_prefixes: Vec<String>,
    pub next_token: Option<String>,
}

/// 列举一页对象。`prefix` 为完整前缀（含配置前缀），`delimiter` 传 `/` 可做目录浏览。
fn list_page(
    client: &reqwest::blocking::Client,
    cfg: &S3Config,
    prefix: &str,
    delimiter: Option<&str>,
    continuation: Option<&str>,
) -> Result<ListPage, String> {
    let base = cfg.bucket_url()?;
    let mut params: Vec<(String, String)> = vec![
        ("list-type".to_string(), "2".to_string()),
        ("max-keys".to_string(), "1000".to_string()),
    ];
    if !prefix.is_empty() {
        params.push(("prefix".to_string(), prefix.to_string()));
    }
    if let Some(d) = delimiter {
        params.push(("delimiter".to_string(), d.to_string()));
    }
    if let Some(t) = continuation {
        params.push(("continuation-token".to_string(), t.to_string()));
    }
    let qs = canonical_query(&params);
    let url = format!("{base}?{qs}");

    let (amz_date, datestamp) = amz_dates(chrono::Utc::now());
    let headers = if cfg.is_anonymous() {
        Vec::new()
    } else {
        signed_get_headers(cfg, &url, &amz_date, &datestamp)?
    };

    let mut req = client.get(&url);
    for (k, v) in headers {
        req = req.header(k, v);
    }
    let resp = req.send().map_err(|e| format!("S3 请求失败: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "S3 列举失败（HTTP {}）: {}",
            status.as_u16(),
            summarize_xml_error(&body)
        ));
    }
    Ok(parse_list_response(&body))
}

/// 解析 `ListObjectsV2` 响应。
pub fn parse_list_response(xml: &str) -> ListPage {
    let doc = match roxmltree::Document::parse(xml) {
        Ok(d) => d,
        Err(_) => return ListPage::default(),
    };
    let mut page = ListPage::default();

    for node in doc.descendants() {
        match node.tag_name().name() {
            "Contents" => {
                let key = child_text(&node, "Key").unwrap_or_default();
                if key.is_empty() {
                    continue;
                }
                page.objects.push(S3Object {
                    key,
                    size: child_text(&node, "Size")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                    last_modified: child_text(&node, "LastModified")
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.timestamp()),
                });
            }
            "CommonPrefixes" => {
                if let Some(p) = child_text(&node, "Prefix") {
                    if !p.is_empty() {
                        page.common_prefixes.push(p);
                    }
                }
            }
            "NextContinuationToken" => {
                page.next_token = node.text().map(str::to_string).filter(|s| !s.is_empty());
            }
            _ => {}
        }
    }
    page
}

fn child_text(node: &roxmltree::Node, name: &str) -> Option<String> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .and_then(|c| c.text())
        .map(str::to_string)
}

/// 抽取 S3 XML 错误里的 `Message`（失败提示更可读）。
fn summarize_xml_error(body: &str) -> String {
    if let Ok(doc) = roxmltree::Document::parse(body) {
        if let Some(msg) = doc
            .descendants()
            .find(|n| n.tag_name().name() == "Message")
            .and_then(|n| n.text())
        {
            return msg.to_string();
        }
    }
    body.chars().take(200).collect()
}

/// 递归列举任意前缀下的全部对象（自动翻页）。`prefix` 为**完整对象键前缀**。
pub fn list_all_objects_under(
    cfg: &S3Config,
    client: &reqwest::blocking::Client,
    prefix: &str,
) -> Result<Vec<S3Object>, String> {
    let mut out = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let page = list_page(client, cfg, prefix, None, token.as_deref())?;
        out.extend(page.objects);
        match page.next_token {
            Some(t) => token = Some(t),
            None => break,
        }
    }
    Ok(out)
}

/// 目录浏览：返回当前层的「子目录前缀」与「对象」。
pub fn list_dir(cfg: &S3Config, dir_prefix: &str) -> Result<Vec<crate::source::DirEntry>, String> {
    let client = build_blocking_client_for(&cfg.endpoint)?;
    let prefix = normalize_dir_prefix(cfg, dir_prefix);
    let page = list_page(&client, cfg, &prefix, Some("/"), None)?;

    let mut entries: Vec<crate::source::DirEntry> = Vec::new();
    for p in page.common_prefixes {
        let rel = cfg.relative_key(p.trim_end_matches('/'));
        entries.push(crate::source::DirEntry {
            name: last_segment(&rel),
            url: rel,
            is_dir: true,
            size: None,
            modified: None,
        });
    }
    for obj in page.objects {
        let rel = cfg.relative_key(&obj.key);
        if rel.is_empty() {
            continue;
        }
        entries.push(crate::source::DirEntry {
            name: last_segment(&rel),
            url: rel,
            is_dir: false,
            size: Some(obj.size),
            modified: obj.last_modified,
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

/// 把浏览路径归一化为完整前缀（末尾带 `/`）。
fn normalize_dir_prefix(cfg: &S3Config, dir_prefix: &str) -> String {
    let dir = dir_prefix.trim_matches('/');
    if dir.is_empty() {
        if cfg.prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", cfg.prefix)
        }
    } else {
        format!("{}/", cfg.full_key(dir))
    }
}

fn last_segment(key: &str) -> String {
    key.rsplit('/').next().unwrap_or(key).to_string()
}

/// 是否为可扫描的音频对象。
pub fn is_audio_key(key: &str) -> bool {
    let name = key.rsplit('/').next().unwrap_or(key);
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("").to_ascii_lowercase();
    AUDIO_EXTS.contains(&ext.as_str())
}

// ---------------------------------------------------------------------------
// 对象读取
// ---------------------------------------------------------------------------

/// 下载对象字节（完整）。用于封面等小文件。
pub fn fetch_bytes(cfg: &S3Config, key: &str) -> Result<Vec<u8>, String> {
    let client = build_blocking_client_for(&cfg.endpoint)?;
    let url = cfg.object_url(&cfg.full_key(key))?;
    let (amz_date, datestamp) = amz_dates(chrono::Utc::now());
    let mut req = client.get(&url);
    if !cfg.is_anonymous() {
        for (k, v) in signed_get_headers(cfg, &url, &amz_date, &datestamp)? {
            req = req.header(k, v);
        }
    }
    let resp = req.send().map_err(|e| format!("S3 读取失败: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("S3 读取失败（HTTP {}）", status.as_u16()));
    }
    resp.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| format!("读取响应失败: {e}"))
}

/// 读取对象头部若干字节（标签探测用）。
fn fetch_head_bytes(
    client: &reqwest::blocking::Client,
    cfg: &S3Config,
    key: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let url = cfg.object_url(key)?;
    let (amz_date, datestamp) = amz_dates(chrono::Utc::now());
    let mut req = client
        .get(&url)
        .header("Range", format!("bytes=0-{}", limit.saturating_sub(1)));
    if !cfg.is_anonymous() {
        for (k, v) in signed_get_headers(cfg, &url, &amz_date, &datestamp)? {
            req = req.header(k, v);
        }
    }
    let resp = req.send().map_err(|e| format!("S3 探测失败: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return Err(format!("S3 探测失败（HTTP {status}）"));
    }
    use std::io::Read;
    let mut buf = Vec::new();
    resp.take(limit)
        .read_to_end(&mut buf)
        .map_err(|e| format!("读取探测响应失败: {e}"))?;
    Ok(buf)
}

/// 读取同名 `.lrc` 侧车歌词对象（无则 None）。`key` 为相对键。
pub fn get_lyrics(config: &StreamServerConfig, key: &str) -> Option<String> {
    let cfg = S3Config::from_stream_config(config).ok()?;
    let lrc_key = swap_extension(key, "lrc");
    let bytes = fetch_bytes(&cfg, &lrc_key).ok()?;
    String::from_utf8(bytes).ok()
}

fn swap_extension(key: &str, ext: &str) -> String {
    match key.rsplit_once('.') {
        Some((base, _)) if !base.is_empty() => format!("{base}.{ext}"),
        _ => format!("{key}.{ext}"),
    }
}

// ---------------------------------------------------------------------------
// 连接测试 / 扫描
// ---------------------------------------------------------------------------

/// 测试连接：列举 1 个对象验证凭据与桶可达。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    let cfg = match S3Config::from_stream_config(config) {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: e,
                server_version: None,
            }
        }
    };
    let client = match build_blocking_client_for(&cfg.endpoint) {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: e,
                server_version: None,
            }
        }
    };
    let prefix = if cfg.prefix.is_empty() {
        String::new()
    } else {
        format!("{}/", cfg.prefix)
    };
    match list_page(&client, &cfg, &prefix, None, None) {
        Ok(_) => ConnectionTestResult {
            success: true,
            message: format!(
                "S3 连接成功（bucket: {}）{}",
                cfg.bucket,
                if cfg.is_anonymous() { " · 匿名" } else { "" }
            ),
            server_version: None,
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

/// 扫描全部音频对象为 `ScannedSong`。
///
/// `read_tags=true` 时逐首拉取头部 1 MiB 用 lofty 解析标签（较慢）；
/// 否则仅用对象键做文件名级兜底（快速）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    scan_prefix(config, "", cover_cache, existing_modified, progress, read_tags)
}

/// 扫描指定目录前缀（相对 `cfg.prefix`，空串=库根）下的音频对象。
///
/// 与 `list_dir` 使用同一套相对键约定，故「浏览某目录 → 加入音乐库」可无缝衔接。
pub fn scan_prefix(
    config: &StreamServerConfig,
    dir_prefix: &str,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let cfg = S3Config::from_stream_config(config)?;
    let client = build_blocking_client_for(&cfg.endpoint)?;

    let prefix = normalize_dir_prefix(&cfg, dir_prefix);
    let objects = list_all_objects_under(&cfg, &client, &prefix)?;
    let audio: Vec<S3Object> = objects.into_iter().filter(|o| is_audio_key(&o.key)).collect();

    let total = audio.len();
    if let Some(tx) = progress.as_ref() {
        let _ = tx.send((0, total));
    }

    // 快速模式：不联网探测，直接用键名兜底。
    if !read_tags {
        let songs = audio
            .iter()
            .map(|o| {
                let rel = cfg.relative_key(&o.key);
                let mut song = crate::utils::webdav::fallback_song_with_modified(&rel, o.last_modified);
                song.file_size = o.size;
                song
            })
            .collect();
        if let Some(tx) = progress.as_ref() {
            let _ = tx.send((total, total));
        }
        return Ok(songs);
    }

    // 探测模式：并行拉头部解析标签。
    use rayon::prelude::*;
    let done = std::sync::atomic::AtomicUsize::new(0);
    let songs: Vec<ScannedSong> = audio
        .par_iter()
        .map(|o| {
            let rel = cfg.relative_key(&o.key);
            // 增量：mtime 未变则跳过网络探测。
            if let (Some(modified), Some(existing)) = (o.last_modified, existing_modified) {
                if existing.get(&rel) == Some(&modified) {
                    let mut s = crate::utils::webdav::fallback_song_with_modified(&rel, Some(modified));
                    s.file_size = o.size;
                    return s;
                }
            }
            let mut song = match fetch_head_bytes(&client, &cfg, &o.key, METADATA_PROBE_BYTES) {
                Ok(bytes) => crate::utils::webdav::parse_probe_bytes(&bytes, &rel, o.last_modified, cover_cache),
                Err(_) => crate::utils::webdav::fallback_song_with_modified(&rel, o.last_modified),
            };
            song.file_size = o.size;
            let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if let Some(tx) = progress.as_ref() {
                let _ = tx.send((n, total));
            }
            song
        })
        .collect();

    Ok(songs)
}

/// 探测单首歌标签（快速扫描后的后台元数据补全）。`key` 为相对键。
/// 仅当探测成功且拿到时长时返回 `Some`。
pub fn probe_song(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    key: &str,
) -> Option<ScannedSong> {
    let cfg = S3Config::from_stream_config(config).ok()?;
    let client = build_blocking_client_for(&cfg.endpoint).ok()?;
    let full = cfg.full_key(key);
    let bytes = fetch_head_bytes(&client, &cfg, &full, METADATA_PROBE_BYTES).ok()?;
    let rel = cfg.relative_key(&full);
    let song = crate::utils::webdav::parse_probe_bytes(&bytes, &rel, None, cover_cache);
    (song.duration > 0.0).then_some(song)
}

/// 下载整首歌到本地缓存，返回缓存路径。
///
/// 复用与 WebDAV 相同的 stream 缓存目录与命名（`webdav::cached_path`），
/// 保证 `get_cached_stream_path(s)` 等前端查询对两种来源一致生效。
pub fn cache_song(
    config: &StreamServerConfig,
    cache_root: &std::path::Path,
    server_id: &str,
    song_id: &str,
) -> Result<String, String> {
    let dir = cache_root.join("webdav");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建缓存目录: {e}"))?;
    let path = crate::utils::webdav::cached_path(cache_root, server_id, song_id);
    if path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(path.to_string_lossy().to_string());
    }

    let cfg = S3Config::from_stream_config(config)?;
    let bytes = fetch_bytes(&cfg, song_id)?;

    // 先写临时文件再重命名，避免半成品缓存。
    let tmp_path = path.with_extension("part");
    if let Err(e) = std::fs::write(&tmp_path, &bytes) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("写入缓存失败: {e}"));
    }
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("落盘缓存失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ServerType;
    fn cfg() -> S3Config {
        S3Config {
            endpoint: "https://s3.example.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "music".to_string(),
            prefix: "lib".to_string(),
            access_key: "AKID".to_string(),
            secret_key: "SECRET".to_string(),
            path_style: true,
        }
    }

    /// AWS 官方 SigV4 测试向量（GET Object 示例）。
    #[test]
    fn sigv4_matches_aws_test_vector() {
        let auth = authorization(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "20130524T000000Z",
            "20130524",
            "GET",
            "/test.txt",
            "",
            "examplebucket.s3.amazonaws.com",
            &[("range".to_string(), "bytes=0-9".to_string())],
            EMPTY_SHA256,
        );
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }

    #[test]
    fn empty_payload_hash_is_shasum_of_empty() {
        assert_eq!(sha256_hex(b""), EMPTY_SHA256);
    }

    #[test]
    fn hmac_sha256_matches_rfc4231_case2() {
        // RFC 4231 Test Case 2
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn uri_encode_escapes_reserved_and_keeps_slashes() {
        assert_eq!(uri_encode("a b/c~d.mp3", false), "a%20b/c~d.mp3");
        assert_eq!(uri_encode("a/b", true), "a%2Fb");
        assert_eq!(uri_encode("é", false), "%C3%A9");
    }

    #[test]
    fn canonical_query_sorts_and_encodes() {
        let qs = canonical_query(&[
            ("prefix".to_string(), "my lib/".to_string()),
            ("list-type".to_string(), "2".to_string()),
        ]);
        assert_eq!(qs, "list-type=2&prefix=my%20lib%2F");
    }

    #[test]
    fn config_parses_bucket_and_prefix() {
        let mut config = StreamServerConfig {
            server_type: ServerType::S3,
            server_name: "n".into(),
            server_url: "https://minio.local:9000/".into(),
            username: "ak".into(),
            password: "sk".into(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: Some("/music/albums/".into()),
            access_token: None,
            user_id: Some("cn-north-1".into()),
        };
        let c = S3Config::from_stream_config(&config).unwrap();
        assert_eq!(c.endpoint, "https://minio.local:9000");
        assert_eq!(c.bucket, "music");
        assert_eq!(c.prefix, "albums");
        assert_eq!(c.region, "cn-north-1");
        assert!(c.path_style, "自建 endpoint 应默认 path-style");
        assert_eq!(c.full_key("a/b.mp3"), "albums/a/b.mp3");
        assert_eq!(c.relative_key("albums/a/b.mp3"), "a/b.mp3");

        // 缺少 bucket → 报错
        config.base_path = None;
        assert!(S3Config::from_stream_config(&config).is_err());
    }

    #[test]
    fn region_detected_from_aws_host() {
        assert_eq!(detect_region("https://s3.us-west-2.amazonaws.com"), "us-west-2");
        assert_eq!(detect_region("https://s3-eu-central-1.amazonaws.com"), "eu-central-1");
        assert_eq!(detect_region("https://minio.local:9000"), "us-east-1");
    }

    #[test]
    fn object_url_path_and_virtual_host() {
        let mut c = cfg();
        assert_eq!(
            c.object_url("a b.mp3").unwrap(),
            "https://s3.example.com/music/a%20b.mp3"
        );
        c.path_style = false;
        assert_eq!(
            c.object_url("a.mp3").unwrap(),
            "https://music.s3.example.com/a.mp3"
        );
    }

    #[test]
    fn anonymous_config_returns_plain_url() {
        let mut c = cfg();
        c.access_key = String::new();
        c.secret_key = String::new();
        assert!(c.is_anonymous());
        let url = presign_get_url_with(&c, "a.mp3", 60, chrono::Utc::now()).unwrap();
        assert_eq!(url, "https://s3.example.com/music/lib/a.mp3");
        assert!(!url.contains("X-Amz-Signature"));
    }

    #[test]
    fn presign_url_carries_scope_and_signature() {
        let c = cfg();
        let now = chrono::DateTime::parse_from_rfc3339("2013-05-24T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let url = presign_get_url_with(&c, "a.mp3", 3600, now).unwrap();
        assert!(url.starts_with("https://s3.example.com/music/lib/a.mp3?"), "{url}");
        assert!(url.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
        assert!(url.contains("X-Amz-Credential=AKID%2F20130524%2Fus-east-1%2Fs3%2Faws4_request"));
        assert!(url.contains("X-Amz-Date=20130524T000000Z"));
        assert!(url.contains("X-Amz-Expires=3600"));
        assert!(url.contains("X-Amz-SignedHeaders=host"));
        assert!(url.contains("&X-Amz-Signature="));
    }

    #[test]
    fn list_response_parses_objects_prefixes_and_token() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>music</Name>
  <Prefix>lib/</Prefix>
  <IsTruncated>true</IsTruncated>
  <NextContinuationToken>TOKEN123</NextContinuationToken>
  <Contents>
    <Key>lib/a.mp3</Key>
    <LastModified>2013-05-24T00:00:00.000Z</LastModified>
    <Size>1024</Size>
  </Contents>
  <Contents>
    <Key>lib/b.flac</Key>
    <Size>2048</Size>
  </Contents>
  <CommonPrefixes><Prefix>lib/sub/</Prefix></CommonPrefixes>
</ListBucketResult>"#;
        let page = parse_list_response(xml);
        assert_eq!(page.objects.len(), 2);
        assert_eq!(page.objects[0].key, "lib/a.mp3");
        assert_eq!(page.objects[0].size, 1024);
        assert_eq!(page.objects[0].last_modified, Some(1369353600));
        assert_eq!(page.objects[1].key, "lib/b.flac");
        assert_eq!(page.objects[1].last_modified, None);
        assert_eq!(page.common_prefixes, vec!["lib/sub/".to_string()]);
        assert_eq!(page.next_token.as_deref(), Some("TOKEN123"));
    }

    #[test]
    fn audio_key_filter() {
        assert!(is_audio_key("lib/a.mp3"));
        assert!(is_audio_key("lib/A.FLAC"));
        assert!(!is_audio_key("lib/cover.jpg"));
        assert!(!is_audio_key("lib/noext"));
    }

    #[test]
    fn swap_extension_rewrites_last_segment() {
        assert_eq!(swap_extension("lib/a.mp3", "lrc"), "lib/a.lrc");
        assert_eq!(swap_extension("lib/noext", "lrc"), "lib/noext.lrc");
    }
}
