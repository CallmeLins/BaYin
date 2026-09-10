//! UPnP / DLNA 媒体服务器音乐源（阶段 B6）。
//!
//! 为什么这一项成本最低：**DLNA 的内容资源本身就是 HTTP URL**（`<res>` 元素），
//! 所以拿到的直链可以直接交给现有播放引擎（`HttpStreamSource`），无需任何桥接；
//! A5 Range 稀疏缓存写透与 A7 可信主机连接放行也一并复用。
//!
//! 本模块只做三件事：
//! 1. **发现**：SSDP M-SEARCH 收集 `LOCATION`，或直接使用用户填写的设备描述 URL；
//! 2. **描述解析**：从设备描述 XML 中取出 `ContentDirectory` 服务的 `controlURL`；
//! 3. **内容浏览**：`ContentDirectory#Browse` 的 SOAP 往返 + DIDL-Lite 解析。
//!
//! `StreamServerConfig` 字段映射：
//! | 字段 | 含义 |
//! |---|---|
//! | `server_url` | 设备描述 URL（如 `http://192.168.1.10:8200/rootDesc.xml`） |
//! | `base_path`  | 可选：起始容器 ObjectID（默认 `0`，即根目录） |
//! | `username` / `password` | 可选 HTTP Basic（少数 NAS 需要） |

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::http::build_blocking_client_for;

/// SSDP 多播地址。
const SSDP_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const SSDP_PORT: u16 = 1900;
/// ContentDirectory 服务类型前缀（版本号由设备描述决定）。
const CONTENT_DIRECTORY: &str = "urn:schemas-upnp-org:service:ContentDirectory";
/// 单次 Browse 请求条数。
const BROWSE_PAGE: u32 = 200;
/// 递归扫描的最大容器数，防止环状引用导致无限递归。
const MAX_CONTAINERS: usize = 2000;
/// 元数据探测读取的头部字节数。
const METADATA_PROBE_BYTES: u64 = 1024 * 1024;

const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "ape", "wv", "aiff", "aif", "alac", "dsf",
    "dff", "mka",
];

// ==================== 设备描述 ====================

/// 从设备描述 XML 解析出的 UPnP 媒体服务器信息。
#[derive(Debug, Clone, PartialEq)]
pub struct UpnpDevice {
    pub friendly_name: String,
    pub udn: String,
    pub manufacturer: Option<String>,
    pub model_name: Option<String>,
    /// ContentDirectory 的绝对控制 URL。
    pub control_url: String,
    /// ContentDirectory 服务类型（含版本，用于 SOAPAction）。
    pub service_type: String,
    /// 设备描述自身的绝对 URL。
    pub description_url: String,
}

/// 解析设备描述 XML。`description_url` 用于把相对的 `controlURL` 解析为绝对 URL。
pub fn parse_device_description(xml: &str, description_url: &str) -> Result<UpnpDevice, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("设备描述 XML 解析失败: {e}"))?;

    let device = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "device")
        .ok_or_else(|| "设备描述中缺少 <device> 节点".to_string())?;

    let child_text = |parent: roxmltree::Node, name: &str| -> Option<String> {
        parent
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == name)
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let friendly_name = child_text(device, "friendlyName").unwrap_or_else(|| "UPnP 服务器".into());
    let udn = child_text(device, "UDN").unwrap_or_default();

    // 查找 ContentDirectory 服务（可能在 device 或内嵌 deviceList 中）。
    let mut control_url_raw: Option<String> = None;
    let mut service_type: Option<String> = None;
    for service in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "service")
    {
        let st = child_text(service, "serviceType").unwrap_or_default();
        if st.starts_with(CONTENT_DIRECTORY) {
            if let Some(cu) = child_text(service, "controlURL") {
                service_type = Some(st);
                control_url_raw = Some(cu);
                break;
            }
        }
    }

    let control_url_raw =
        control_url_raw.ok_or_else(|| "该设备未提供 ContentDirectory 服务".to_string())?;
    let control_url = resolve_url(description_url, &control_url_raw)
        .ok_or_else(|| format!("无法解析 controlURL: {control_url_raw}"))?;

    Ok(UpnpDevice {
        friendly_name,
        udn,
        manufacturer: child_text(device, "manufacturer"),
        model_name: child_text(device, "modelName"),
        control_url,
        // 缺省按 v1（几乎所有 DLNA 服务器都兼容 v1 的 SOAPAction）。
        service_type: service_type.unwrap_or_else(|| format!("{CONTENT_DIRECTORY}:1")),
        description_url: description_url.to_string(),
    })
}

/// 把可能的相对 URL 解析为绝对 URL。
fn resolve_url(base: &str, target: &str) -> Option<String> {
    let target = target.trim();
    if target.starts_with("http://") || target.starts_with("https://") {
        return Some(target.to_string());
    }
    let base = url::Url::parse(base).ok()?;
    base.join(target).ok().map(|u| u.to_string())
}

// ==================== SSDP 发现 ====================

/// SSDP 发现到的候选设备（尚未确认是否含 ContentDirectory）。
#[derive(Debug, Clone, PartialEq)]
pub struct SsdpLocation {
    pub location: String,
    pub server: Option<String>,
    pub usn: Option<String>,
}

/// 发送 SSDP M-SEARCH 并收集响应中的 `LOCATION`（best-effort）。
///
/// 需要局域网多播可用；失败时返回空列表而非错误——用户始终可以手填描述 URL。
pub fn ssdp_discover(timeout: Duration) -> Vec<SsdpLocation> {
    let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(400)));

    let msg = "M-SEARCH * HTTP/1.1\r\n\
               HOST: 239.255.255.250:1900\r\n\
               MAN: \"ssdp:discover\"\r\n\
               MX: 2\r\n\
               ST: urn:schemas-upnp-org:device:MediaServer:1\r\n\
               \r\n";
    let target = SocketAddrV4::new(SSDP_ADDR, SSDP_PORT);
    if socket.send_to(msg.as_bytes(), target).is_err() {
        return Vec::new();
    }

    let deadline = Instant::now() + timeout;
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<SsdpLocation> = Vec::new();
    let mut buf = [0u8; 4096];

    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let headers = parse_ssdp_headers(&text);
                if let Some(location) = headers.get("location") {
                    if !seen.iter().any(|s| s == location) {
                        seen.push(location.clone());
                        out.push(SsdpLocation {
                            location: location.clone(),
                            server: headers.get("server").cloned(),
                            usn: headers.get("usn").cloned(),
                        });
                    }
                }
            }
            Err(_) => {
                // 读超时：继续等到总超时（可能还有设备陆续响应）。
                if Instant::now() >= deadline {
                    break;
                }
            }
        }
    }
    out
}

/// 解析 SSDP 响应头（大小写不敏感，键统一小写）。纯函数，便于离线单测。
pub fn parse_ssdp_headers(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in text.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }
            out.entry(key).or_insert_with(|| v.trim().to_string());
        }
    }
    out
}

// ==================== 设备发现（配置页「自动发现」按钮） ====================

/// 发现到的可用 UPnP 媒体服务器（已确认提供 `ContentDirectory`）。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    /// 设备友好名，可直接用作配置里的「名称」。
    pub friendly_name: String,
    /// 设备描述 URL —— 直接填入配置里的「地址」字段即可。
    pub location: String,
    pub udn: String,
    pub manufacturer: Option<String>,
    pub model_name: Option<String>,
}

/// 探测单个设备描述的超时（发现流程专用，须远短于常规请求）。
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(800);
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// SSDP 发现 + 描述校验，只返回真正提供 `ContentDirectory` 的设备。
///
/// 局域网里同一个网段往往还有路由器、打印机、智能音箱等一堆会响应 SSDP 的
/// 设备，其中大部分**没有** `ContentDirectory`。因此这里必须逐个拉设备描述
/// 校验，且任何单台设备失败都只跳过它自己。
pub fn discover_devices(timeout: Duration) -> Vec<DiscoveredDevice> {
    let probed: Vec<(String, Result<UpnpDevice, String>)> = ssdp_discover(timeout)
        .into_iter()
        .map(|loc| {
            let result = probe_device(&loc.location);
            (loc.location, result)
        })
        .collect();
    collect_devices(probed)
}

/// 拉取并解析单个设备描述（发现流程专用：短超时、不写设备缓存）。
fn probe_device(location: &str) -> Result<UpnpDevice, String> {
    let client = crate::utils::http::build_blocking_client_with_timeout(
        location,
        PROBE_CONNECT_TIMEOUT,
        PROBE_TIMEOUT,
    )?;
    let resp = client
        .get(location)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let body = resp.text().map_err(|e| format!("读取响应失败: {e}"))?;
    // 无 ContentDirectory 的设备会在这里被拒绝（解析器负责判定）。
    parse_device_description(&body, location)
}

/// 把「描述 URL → 解析结果」合并为按 UDN 去重的设备列表。
///
/// 纯函数，便于离线单测覆盖"跳过非媒体设备""同设备多地址去重"等分支。
fn collect_devices(probed: Vec<(String, Result<UpnpDevice, String>)>) -> Vec<DiscoveredDevice> {
    let mut out: Vec<DiscoveredDevice> = Vec::new();
    let mut seen_udn: Vec<String> = Vec::new();
    for (location, result) in probed {
        match result {
            Ok(dev) => {
                // 同一台设备可能从多个网卡/多个 LOCATION 被重复发现。
                if seen_udn.iter().any(|u| u == &dev.udn) {
                    continue;
                }
                seen_udn.push(dev.udn.clone());
                out.push(DiscoveredDevice {
                    friendly_name: dev.friendly_name,
                    location: dev.description_url,
                    udn: dev.udn,
                    manufacturer: dev.manufacturer,
                    model_name: dev.model_name,
                });
            }
            Err(e) => log::debug!("跳过非媒体设备 {location}: {e}"),
        }
    }
    out
}

// ==================== Browse（SOAP） ====================

/// 构造 `ContentDirectory#Browse` 的 SOAP 请求体。
pub fn browse_envelope(service_type: &str, object_id: &str, start: u32, count: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:Browse xmlns:u="{service_type}">
<ObjectID>{}</ObjectID>
<BrowseFlag>BrowseDirectChildren</BrowseFlag>
<Filter>*</Filter>
<StartingIndex>{start}</StartingIndex>
<RequestedCount>{count}</RequestedCount>
<SortCriteria></SortCriteria>
</u:Browse>
</s:Body>
</s:Envelope>"#,
        xml_escape(object_id)
    )
}

/// 转义 XML 文本节点内容。
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// DIDL-Lite 中的一个对象（容器或条目）。
#[derive(Debug, Clone, PartialEq)]
pub struct DidlEntry {
    pub id: String,
    pub parent_id: String,
    pub title: String,
    pub is_container: bool,
    /// 音频资源直链（HTTP）。
    pub res_url: Option<String>,
    pub size: Option<u64>,
    pub duration_secs: Option<u64>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// 封面图 URL（`upnp:albumArtURI`）。
    pub cover_url: Option<String>,
    /// `upnp:class`，如 `object.item.audioItem.musicTrack`。
    pub class: Option<String>,
}

impl DidlEntry {
    /// 是否为可播放的音频条目。
    ///
    /// 判定顺序：`upnp:class` 含 `audioItem` → 资源扩展名属于音频 → 兜底 false。
    pub fn is_audio(&self) -> bool {
        if self.is_container {
            return false;
        }
        if let Some(class) = &self.class {
            if class.contains("audioItem") {
                return true;
            }
        }
        self.res_url
            .as_deref()
            .map(extension_of)
            .map(|ext| AUDIO_EXTS.contains(&ext.as_str()))
            .unwrap_or(false)
    }
}

/// 取 URL 路径部分的扩展名（小写，无点）。忽略查询串。
pub fn extension_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// 解析 `Browse` 响应：返回 (条目, 总匹配数)。
pub fn parse_browse_response(xml: &str) -> Result<(Vec<DidlEntry>, u32), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("Browse 响应解析失败: {e}"))?;

    // SOAP Fault 要把错误信息透出来，否则用户只看到"0 首"。
    if let Some(fault) = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Fault")
    {
        // UPnP 的 `errorDescription` 比泛化的 `faultstring`（通常只是 "UPnPError"）
        // 具体得多，因此优先取它。
        let pick = |name: &str| -> Option<String> {
            fault
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == name)
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
        };
        let msg = pick("errorDescription")
            .or_else(|| pick("faultstring"))
            .unwrap_or_else(|| "UPnP Browse 失败".to_string());
        return Err(msg);
    }

    let result_node = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Result")
        .ok_or_else(|| "Browse 响应缺少 <Result>".to_string())?;

    // roxmltree 已完成实体解码，`text()` 即为原始的 DIDL-Lite XML。
    let didl = result_node.text().unwrap_or("");
    let entries = parse_didl(didl);

    let total = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "TotalMatches")
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse::<u32>().ok())
        .unwrap_or(entries.len() as u32);

    Ok((entries, total))
}

/// 解析 DIDL-Lite XML 片段。
pub fn parse_didl(xml: &str) -> Vec<DidlEntry> {
    let xml = xml.trim();
    if xml.is_empty() {
        return Vec::new();
    }
    let doc = match roxmltree::Document::parse(xml) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let child_text = |parent: roxmltree::Node, name: &str| -> Option<String> {
        parent
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == name)
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let mut out = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let kind = node.tag_name().name();
        let is_container = match kind {
            "container" => true,
            "item" => false,
            _ => continue,
        };

        let res = node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "res");

        out.push(DidlEntry {
            id: node.attribute("id").unwrap_or_default().to_string(),
            parent_id: node.attribute("parentID").unwrap_or_default().to_string(),
            title: child_text(node, "title").unwrap_or_else(|| "(未命名)".to_string()),
            is_container,
            res_url: res.and_then(|r| r.text()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            size: res
                .and_then(|r| r.attribute("size"))
                .and_then(|v| v.trim().parse::<u64>().ok()),
            duration_secs: res
                .and_then(|r| r.attribute("duration"))
                .and_then(parse_duration_secs),
            artist: child_text(node, "artist").or_else(|| child_text(node, "creator")),
            album: child_text(node, "album"),
            cover_url: child_text(node, "albumArtURI"),
            class: child_text(node, "class"),
        });
    }
    out
}

/// 解析 UPnP 的 `H:MM:SS[.fff]` 时长。
pub fn parse_duration_secs(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let head = raw.split('.').next().unwrap_or(raw);
    let parts: Vec<&str> = head.split(':').collect();
    let (h, m, s) = match parts.as_slice() {
        [h, m, s] => (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?, s.parse::<u64>().ok()?),
        [m, s] => (0, m.parse::<u64>().ok()?, s.parse::<u64>().ok()?),
        [s] => (0, 0, s.parse::<u64>().ok()?),
        _ => return None,
    };
    Some(h * 3600 + m * 60 + s)
}

// ==================== 网络访问 ====================

/// 可选 HTTP Basic 头（DLNA 服务器通常允许匿名，少数 NAS 需要认证）。
fn auth_headers(config: &StreamServerConfig) -> Option<reqwest::header::HeaderMap> {
    if config.username.is_empty() && config.password.is_empty() {
        return None;
    }
    Some(crate::utils::webdav::build_auth_headers(config))
}

/// 设备描述缓存：`server_url` → 已解析设备（避免每次调用都重新拉描述）。
fn device_cache() -> &'static Mutex<HashMap<String, UpnpDevice>> {
    static CACHE: OnceLock<Mutex<HashMap<String, UpnpDevice>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取设备描述（带缓存）。
pub fn fetch_device(config: &StreamServerConfig) -> Result<UpnpDevice, String> {
    let key = config.server_url.trim().to_string();
    if key.is_empty() {
        return Err("未填写 UPnP 设备描述 URL".to_string());
    }
    if let Ok(cache) = device_cache().lock() {
        if let Some(dev) = cache.get(&key) {
            return Ok(dev.clone());
        }
    }

    let client = build_blocking_client_for(&key)?;
    let mut req = client.get(&key);
    if let Some(h) = auth_headers(config) {
        req = req.headers(h);
    }
    let resp = req.send().map_err(|e| format!("获取设备描述失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("获取设备描述失败，状态码 {}", resp.status().as_u16()));
    }
    let body = resp.text().map_err(|e| format!("读取设备描述失败: {e}"))?;
    let device = parse_device_description(&body, &key)?;

    if let Ok(mut cache) = device_cache().lock() {
        cache.insert(key, device.clone());
    }
    Ok(device)
}

/// 清空设备描述缓存（设置变更后调用）。
pub fn invalidate_device_cache() {
    if let Ok(mut cache) = device_cache().lock() {
        cache.clear();
    }
}

/// 浏览某个容器的直接子项。
pub fn browse(config: &StreamServerConfig, object_id: &str, start: u32, count: u32) -> Result<(Vec<DidlEntry>, u32), String> {
    let device = fetch_device(config)?;
    let body = browse_envelope(&device.service_type, object_id, start, count);
    let client = build_blocking_client_for(&device.control_url)?;
    let soap_action = format!("\"{}#Browse\"", device.service_type);

    let mut req = client
        .post(&device.control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", soap_action)
        .body(body);
    if let Some(h) = auth_headers(config) {
        req = req.headers(h);
    }
    let resp = req.send().map_err(|e| format!("Browse 请求失败: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.text().map_err(|e| format!("读取 Browse 响应失败: {e}"))?;
    // SOAP Fault 通常仍是 500，但响应体里有可读原因——交给解析器透传。
    if !(200..300).contains(&status) && !text.contains("Fault") {
        return Err(format!("Browse 失败，状态码 {status}"));
    }
    parse_browse_response(&text)
}

/// 一次性拉完整容器（自动翻页）。
pub fn browse_all(config: &StreamServerConfig, object_id: &str) -> Result<Vec<DidlEntry>, String> {
    let mut out = Vec::new();
    let mut start = 0u32;
    loop {
        let (page, total) = browse(config, object_id, start, BROWSE_PAGE)?;
        let n = page.len() as u32;
        out.extend(page);
        start += n;
        if n == 0 || start >= total || n < BROWSE_PAGE {
            break;
        }
    }
    Ok(out)
}

/// 根容器 ID（配置了 `base_path` 则从该容器开始）。
pub fn root_object_id(config: &StreamServerConfig) -> String {
    config
        .base_path
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("0")
        .to_string()
}

/// 测试连接。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    match fetch_device(config) {
        Ok(dev) => ConnectionTestResult {
            success: true,
            message: format!("已连接到 {}", dev.friendly_name),
            server_version: dev.model_name.or(dev.manufacturer),
        },
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

/// 列出某个容器的子项（目录浏览用；`dir_url` 为空取根容器）。
pub fn list_dir(config: &StreamServerConfig, dir_url: Option<&str>) -> Result<Vec<crate::source::DirEntry>, String> {
    let object_id = dir_url
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| root_object_id(config));

    let entries = browse_all(config, &object_id)?;
    Ok(entries
        .into_iter()
        .map(|e| {
            let is_audio = e.is_audio();
            crate::source::DirEntry {
                name: e.title,
                // 容器用 ObjectID 作为"URL"，条目用资源直链。
                url: if e.is_container {
                    e.id
                } else {
                    e.res_url.clone().unwrap_or_default()
                },
                is_dir: e.is_container,
                size: if is_audio { e.size } else { None },
                modified: None,
            }
        })
        .collect())
}

/// 读取远端字节（封面等，直接 HTTP GET）。
pub fn fetch_bytes(config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
    let client = build_blocking_client_for(url)?;
    let mut req = client.get(url);
    if let Some(h) = auth_headers(config) {
        req = req.headers(h);
    }
    let resp = req.send().map_err(|e| format!("下载失败: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return Err(format!("下载失败，状态码 {status}"));
    }
    resp.bytes().map(|b| b.to_vec()).map_err(|e| format!("读取失败: {e}"))
}

/// 把 DLNA 条目转成库内歌曲（`id` 即资源直链，播放时原样返回）。
pub fn to_scanned_song(entry: &DidlEntry) -> Option<ScannedSong> {
    let url = entry.res_url.clone()?;
    if url.is_empty() {
        return None;
    }
    Some(ScannedSong {
        id: url.clone(),
        title: entry.title.clone(),
        artist: entry
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string()),
        album: entry
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string()),
        duration: entry.duration_secs.unwrap_or(0) as f64,
        file_path: String::new(),
        file_size: entry.size.unwrap_or(0),
        cover_url: entry.cover_url.clone(),
        cover_hash: None,
        file_modified: None,
        is_hr: None,
        is_sq: None,
        format: Some(extension_of(&url)),
        bit_depth: None,
        sample_rate: None,
        bitrate: None,
        channels: None,
        created_at: None,
    })
}

/// 递归浏览全库（用于"扫描音乐库"）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    progress: Option<crate::source::ScanProgress>,
) -> Result<Vec<ScannedSong>, String> {
    let root = root_object_id(config);
    let mut queue = vec![root];
    let mut visited = 0usize;
    let mut out: Vec<ScannedSong> = Vec::new();

    while let Some(container) = queue.pop() {
        visited += 1;
        if visited > MAX_CONTAINERS {
            log::warn!("[UPnP] 容器数超过 {MAX_CONTAINERS}，提前停止递归");
            break;
        }
        let entries = browse_all(config, &container)?;
        for entry in entries {
            if entry.is_container {
                queue.push(entry.id.clone());
            } else if entry.is_audio() {
                if let Some(song) = to_scanned_song(&entry) {
                    out.push(song);
                }
            }
        }
        if let Some(tx) = &progress {
            let _ = tx.send((out.len(), out.len()));
        }
    }
    Ok(out)
}

/// 目录扫描（"加入音乐库"）：扫一个容器即可，不递归。
pub fn scan_dir(
    config: &StreamServerConfig,
    existing_modified: Option<&HashMap<String, i64>>,
    dir_url: &str,
) -> Result<Vec<ScannedSong>, String> {
    let object_id = if dir_url.trim().is_empty() {
        root_object_id(config)
    } else {
        dir_url.trim().to_string()
    };
    let entries = browse_all(config, &object_id)?;
    let mut songs: Vec<ScannedSong> = entries
        .iter()
        .filter(|e| e.is_audio())
        .filter_map(to_scanned_song)
        .collect();

    // 增量：DLNA 一般不提供 mtime，这里按 URL 去重复用库内已有值（保持不变）。
    if let Some(existing) = existing_modified {
        for s in songs.iter_mut() {
            if let Some(m) = existing.get(&s.id) {
                s.file_modified = Some(*m);
            }
        }
    }
    Ok(songs)
}

/// 探测单个资源（用于快速扫描后的元数据补全）。
///
/// DLNA 的 `<res>` 通常已带 `duration` / `size`，因此这一步只在缺失时对资源直链
/// 做一次头部 Range 探测，交 lofty 解析标签。
pub fn probe_song(
    config: &StreamServerConfig,
    cover_cache: Option<&crate::utils::cover::CoverCache>,
    song_url: &str,
) -> Option<ScannedSong> {
    let client = build_blocking_client_for(song_url).ok()?;
    let mut req = client
        .get(song_url)
        .header("Range", format!("bytes=0-{}", METADATA_PROBE_BYTES - 1));
    if let Some(h) = auth_headers(config) {
        req = req.headers(h);
    }
    let resp = req.send().ok()?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return None;
    }
    let bytes = resp.bytes().ok()?.to_vec();
    let song = crate::utils::webdav::parse_probe_bytes(&bytes, song_url, None, cover_cache);
    (song.duration > 0.0).then_some(song)
}

/// 下载整首歌到本地缓存。
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

    let client = build_blocking_client_for(song_id)?;
    let mut req = client.get(song_id);
    if let Some(h) = auth_headers(config) {
        req = req.headers(h);
    }
    let resp = req.send().map_err(|e| format!("下载失败: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return Err(format!("下载失败，状态码 {status}"));
    }

    let tmp_path = path.with_extension("part");
    let mut out = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
    {
        use std::io::Read;
        std::io::copy(&mut resp.take(u64::MAX), &mut out).map_err(|e| format!("写入缓存失败: {e}"))?;
    }
    drop(out);
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("缓存完成失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_DESC: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <device>
    <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
    <friendlyName>飞牛 NAS 音乐</friendlyName>
    <manufacturer>FeiNiu</manufacturer>
    <modelName>fnOS DLNA</modelName>
    <UDN>uuid:1234-5678</UDN>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
        <controlURL>/upnp/control/ConnectionManager</controlURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
        <controlURL>/upnp/control/ContentDirectory</controlURL>
      </service>
    </serviceList>
  </device>
</root>"#;

    #[test]
    fn parses_device_description_and_resolves_control_url() {
        let dev = parse_device_description(ROOT_DESC, "http://192.168.1.10:8200/rootDesc.xml").unwrap();
        assert_eq!(dev.friendly_name, "飞牛 NAS 音乐");
        assert_eq!(dev.udn, "uuid:1234-5678");
        assert_eq!(dev.manufacturer.as_deref(), Some("FeiNiu"));
        assert_eq!(
            dev.control_url,
            "http://192.168.1.10:8200/upnp/control/ContentDirectory"
        );
        assert_eq!(dev.service_type, "urn:schemas-upnp-org:service:ContentDirectory:1");
        // 不能误选 ConnectionManager。
        assert!(!dev.control_url.contains("ConnectionManager"));
    }

    #[test]
    fn device_without_content_directory_is_rejected() {
        let xml = r#"<root><device><friendlyName>X</friendlyName>
            <serviceList><service><serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
            <controlURL>/cm</controlURL></service></serviceList></device></root>"#;
        let err = parse_device_description(xml, "http://h/d.xml").unwrap_err();
        assert!(err.contains("ContentDirectory"), "错误信息应点明缺服务: {err}");
    }

    #[test]
    fn absolute_control_url_is_kept_as_is() {
        let xml = ROOT_DESC.replace("/upnp/control/ContentDirectory", "http://10.0.0.5:9999/cd");
        let dev = parse_device_description(&xml, "http://192.168.1.10:8200/rootDesc.xml").unwrap();
        assert_eq!(dev.control_url, "http://10.0.0.5:9999/cd");
    }

    #[test]
    fn browse_envelope_contains_required_fields() {
        let env = browse_envelope("urn:schemas-upnp-org:service:ContentDirectory:1", "64", 200, 100);
        assert!(env.contains("<ObjectID>64</ObjectID>"));
        assert!(env.contains("<BrowseFlag>BrowseDirectChildren</BrowseFlag>"));
        assert!(env.contains("<StartingIndex>200</StartingIndex>"));
        assert!(env.contains("<RequestedCount>100</RequestedCount>"));
        assert!(env.contains("xmlns:u=\"urn:schemas-upnp-org:service:ContentDirectory:1\""));
    }

    #[test]
    fn browse_envelope_escapes_object_id() {
        let env = browse_envelope("urn:x:1", "a&b<c", 0, 10);
        assert!(env.contains("<ObjectID>a&amp;b&lt;c</ObjectID>"));
    }

    const DIDL: &str = r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/"
        xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">
      <container id="1" parentID="0" restricted="1">
        <dc:title>专辑 A</dc:title>
        <upnp:class>object.container.storageFolder</upnp:class>
      </container>
      <item id="2" parentID="1" restricted="1">
        <dc:title>歌名</dc:title>
        <upnp:class>object.item.audioItem.musicTrack</upnp:class>
        <upnp:artist>歌手</upnp:artist>
        <upnp:album>专辑 A</upnp:album>
        <upnp:albumArtURI>http://nas/cover.jpg</upnp:albumArtURI>
        <res size="41943040" duration="0:03:45.500" protocolInfo="http-get:*:audio/flac:*">http://nas/a/song.flac</res>
      </item>
      <item id="3" parentID="1">
        <dc:title>封面</dc:title>
        <res size="100">http://nas/a/cover.jpg</res>
      </item>
    </DIDL-Lite>"#;

    #[test]
    fn parses_didl_entries() {
        let entries = parse_didl(DIDL);
        assert_eq!(entries.len(), 3);

        let c = &entries[0];
        assert!(c.is_container);
        assert_eq!(c.title, "专辑 A");
        assert_eq!(c.id, "1");
        assert!(!c.is_audio());

        let s = &entries[1];
        assert!(!s.is_container);
        assert_eq!(s.title, "歌名");
        assert_eq!(s.res_url.as_deref(), Some("http://nas/a/song.flac"));
        assert_eq!(s.size, Some(41943040));
        assert_eq!(s.duration_secs, Some(3 * 60 + 45));
        assert_eq!(s.artist.as_deref(), Some("歌手"));
        assert_eq!(s.album.as_deref(), Some("专辑 A"));
        assert_eq!(s.cover_url.as_deref(), Some("http://nas/cover.jpg"));
        assert!(s.is_audio());

        // 非音频资源：class 缺失且扩展名不是音频 → 不当作歌曲。
        assert!(!entries[2].is_audio());
    }

    #[test]
    fn duration_parsing_variants() {
        assert_eq!(parse_duration_secs("0:03:45.500"), Some(225));
        assert_eq!(parse_duration_secs("1:00:00"), Some(3600));
        assert_eq!(parse_duration_secs("3:45"), Some(225));
        assert_eq!(parse_duration_secs("45"), Some(45));
        assert_eq!(parse_duration_secs(""), None);
        assert_eq!(parse_duration_secs("bogus"), None);
        assert_eq!(parse_duration_secs("1:2:3:4"), None);
    }

    #[test]
    fn parses_browse_response_with_escaped_result() {
        let escaped = DIDL
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let xml = format!(
            r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
 <s:Body>
  <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
   <Result>{escaped}</Result>
   <NumberReturned>3</NumberReturned>
   <TotalMatches>3</TotalMatches>
   <UpdateID>1</UpdateID>
  </u:BrowseResponse>
 </s:Body>
</s:Envelope>"#
        );
        let (entries, total) = parse_browse_response(&xml).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(total, 3);
        assert_eq!(entries[1].title, "歌名");
    }

    #[test]
    fn soap_fault_surfaces_reason() {
        let xml = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
          <s:Body><s:Fault>
            <faultcode>s:Client</faultcode>
            <faultstring>UPnPError</faultstring>
            <detail><UPnPError><errorCode>701</errorCode>
            <errorDescription>No such object</errorDescription></UPnPError></detail>
          </s:Fault></s:Body></s:Envelope>"#;
        let err = parse_browse_response(xml).unwrap_err();
        assert_eq!(err, "No such object");
    }

    #[test]
    fn empty_didl_yields_no_entries() {
        assert!(parse_didl("").is_empty());
        assert!(parse_didl("   ").is_empty());
        assert!(parse_didl("<DIDL-Lite></DIDL-Lite>").is_empty());
    }

    #[test]
    fn ssdp_header_parsing_is_case_insensitive() {
        let resp = "HTTP/1.1 200 OK\r\nLOCATION: http://192.168.1.10:8200/rootDesc.xml\r\n\
                    Server: Linux/5.0 UPnP/1.0 DLNADOC/1.50\r\nUSN: uuid:abc::urn:schemas-upnp-org:device:MediaServer:1\r\n\r\n";
        let h = parse_ssdp_headers(resp);
        assert_eq!(
            h.get("location").map(String::as_str),
            Some("http://192.168.1.10:8200/rootDesc.xml")
        );
        assert!(h.get("server").unwrap().contains("DLNADOC"));
        assert!(h.get("usn").unwrap().contains("MediaServer"));
    }

    #[test]
    fn extension_of_ignores_query_and_case() {
        assert_eq!(extension_of("http://n/a/SONG.FLAC"), "flac");
        assert_eq!(extension_of("http://n/a/song.mp3?token=1"), "mp3");
        assert_eq!(extension_of("http://n/a/noext"), "");
    }

    #[test]
    fn root_object_id_defaults_to_zero_and_honours_base_path() {
        let base = StreamServerConfig {
            server_type: ServerType::Upnp,
            server_name: "t".to_string(),
            server_url: "http://192.168.1.10:8200/rootDesc.xml".to_string(),
            username: String::new(),
            password: String::new(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        };

        let mut c = base.clone();
        assert_eq!(root_object_id(&c), "0");

        c.base_path = Some("  64 ".to_string());
        assert_eq!(root_object_id(&c), "64");

        c.base_path = Some("   ".to_string());
        assert_eq!(root_object_id(&c), "0");
    }

    /// 匿名配置不应产生 Authorization 头；填了凭据才生成。
    #[test]
    fn auth_headers_only_when_credentials_present() {
        let mut c = StreamServerConfig {
            server_type: ServerType::Upnp,
            server_name: "t".to_string(),
            server_url: "http://h/d.xml".to_string(),
            username: String::new(),
            password: String::new(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        };
        assert!(auth_headers(&c).is_none());
        c.username = "u".to_string();
        c.password = "p".to_string();
        let h = auth_headers(&c).expect("凭据存在时应生成头");
        assert!(h.contains_key(reqwest::header::AUTHORIZATION));
    }

    use crate::models::ServerType;

    // ---------- 设备发现合并逻辑（离线单测，不依赖真实 SSDP 多播） ----------

    fn device(udn: &str, name: &str, url: &str) -> UpnpDevice {
        UpnpDevice {
            friendly_name: name.to_string(),
            udn: udn.to_string(),
            manufacturer: Some("FeiNiu".to_string()),
            model_name: Some("fnOS DLNA".to_string()),
            control_url: format!("{url}/upnp/control/ContentDirectory"),
            service_type: "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
            description_url: url.to_string(),
        }
    }

    /// 没有 ContentDirectory 的设备（路由器、打印机等）必须被跳过，
    /// 但**不能**影响同批次里的正常设备。
    #[test]
    fn discovery_skips_non_media_devices() {
        let probed = vec![
            (
                "http://192.168.1.1/rootDesc.xml".to_string(),
                Err("该设备未提供 ContentDirectory 服务".to_string()),
            ),
            (
                "http://192.168.1.10:8200/rootDesc.xml".to_string(),
                Ok(device(
                    "uuid:aaaa",
                    "飞牛 NAS 音乐",
                    "http://192.168.1.10:8200/rootDesc.xml",
                )),
            ),
        ];

        let found = collect_devices(probed);
        assert_eq!(found.len(), 1, "非媒体设备应被跳过");
        assert_eq!(found[0].friendly_name, "飞牛 NAS 音乐");
        assert_eq!(found[0].location, "http://192.168.1.10:8200/rootDesc.xml");
        assert_eq!(found[0].udn, "uuid:aaaa");
    }

    /// 同一台设备可能从多个网卡/多个 LOCATION 被重复发现，应按 UDN 去重。
    #[test]
    fn discovery_deduplicates_by_udn() {
        let probed = vec![
            (
                "http://192.168.1.10:8200/rootDesc.xml".to_string(),
                Ok(device(
                    "uuid:aaaa",
                    "飞牛 NAS 音乐",
                    "http://192.168.1.10:8200/rootDesc.xml",
                )),
            ),
            (
                "http://10.0.0.5:8200/rootDesc.xml".to_string(),
                Ok(device(
                    "uuid:aaaa",
                    "飞牛 NAS 音乐",
                    "http://10.0.0.5:8200/rootDesc.xml",
                )),
            ),
            (
                "http://192.168.1.20:8200/rootDesc.xml".to_string(),
                Ok(device(
                    "uuid:bbbb",
                    "群晖 Audio Station",
                    "http://192.168.1.20:8200/rootDesc.xml",
                )),
            ),
        ];

        let found = collect_devices(probed);
        assert_eq!(found.len(), 2, "同一 UDN 只应出现一次");
        // 保留先发现的那台（顺序稳定，便于 UI 展示）。
        assert_eq!(found[0].location, "http://192.168.1.10:8200/rootDesc.xml");
        assert_eq!(found[1].udn, "uuid:bbbb");
    }

    /// 全部探测失败时必须返回空列表而不是 panic（发现失败是可接受的常态）。
    #[test]
    fn discovery_tolerates_total_failure() {
        let probed = vec![
            ("http://a/d.xml".to_string(), Err("超时".to_string())),
            ("http://b/d.xml".to_string(), Err("HTTP 404".to_string())),
        ];
        assert!(collect_devices(probed).is_empty());
        assert!(collect_devices(Vec::new()).is_empty());
    }

    /// 前端按 camelCase 消费，字段名不能漂移。
    #[test]
    fn discovered_device_serializes_as_camel_case() {
        let d = DiscoveredDevice {
            friendly_name: "NAS".to_string(),
            location: "http://h/d.xml".to_string(),
            udn: "uuid:1".to_string(),
            manufacturer: None,
            model_name: Some("m".to_string()),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"friendlyName\""), "{json}");
        assert!(json.contains("\"location\""), "{json}");
        assert!(json.contains("\"modelName\""), "{json}");
        assert!(!json.contains("friendly_name"), "{json}");
    }
}
