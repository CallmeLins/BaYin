//! 本地 HTTP + Range 桥接（`127.0.0.1`）。
//!
//! ## 为什么需要它
//!
//! 播放引擎（[`crate::audio_engine::http_source`]）只会说 HTTP：它靠 `Range:
//! bytes=N-` 做 seek、靠 `Content-Length` 判断总长、靠连续读取驱动解码。**非
//! HTTP 来源**（FTP、以及任何"只能顺序读"的协议）若想接入播放器、A5 Range
//! 稀疏缓存写透与统一 seek 语义，就必须在本地暴露一个支持 Range 的 HTTP 端点。
//!
//! ## 结构
//!
//! - [`RangeReader`]：与协议无关的随机读取抽象。新增一个非 HTTP 源 = 实现一个
//!   `RangeReader`，其余（HTTP 语义、Range、缓存、seek）全部复用。
//! - [`SpoolReader`]：通用的"渐进式落盘"读取器——后台线程从远端顺序拉取写入
//!   临时文件，`read_at` 从该文件随机读取；请求落在已下载区间之外时**从该偏移
//!   重新开一段**，因此 seek 不需要先下载前半段。
//! - 一个基于 `std::net::TcpListener` 的极小 HTTP/1.1 服务：`GET /<token>/<name>`，
//!   无 Range → `200`，有 Range → `206 + Content-Range`，未知 token → `404`。
//!
//! 桥接只监听回环地址，不对外暴露；URL 中的 token 由配置 + 曲目哈希得到，语义上
//! 等价于"这一台服务器上的这一首歌"。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::Instant;

use crate::models::StreamServerConfig;
use sha2::{Digest, Sha256};

/// 单次读取块大小。
const BRIDGE_CHUNK: usize = 64 * 1024;
/// 读取器缓存上限（超出后淘汰最久未用的一个）。
const MAX_CACHED_READERS: usize = 16;

// ==================== 读取器抽象 ====================

/// 与协议无关的随机读取器。
pub trait RangeReader: Send {
    /// 总长度；未知返回 `None`（此时 HTTP 响应不带 `Content-Length`，靠连接关闭定界）。
    fn total_len(&self) -> Option<u64>;

    /// 从 `offset` 读取最多 `buf.len()` 字节。
    ///
    /// 返回 `Ok(0)` 表示已到末尾；`Err` 表示该位置不可读（会转成 HTTP 500）。
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, String>;
}

/// 「从某偏移开始顺序拉取远端数据并写入 sink」的开销函数。
///
/// - `offset`：远端文件中的起始绝对偏移；
/// - `sink`：写入目标（`SpoolReader` 传入临时文件句柄）；
/// - `progress`：每写入一段就调用 `progress(已写字节数)`，用于唤醒等待的读者；
/// - `abort`：置位后应尽快返回 `Ok(())`（部分数据已写入是允许的）。
pub type SegmentOpener =
    dyn Fn(u64, &mut dyn Write, &dyn Fn(u64), &AtomicBool) -> Result<(), String> + Send + Sync;

/// 上游请求头生成器，**每次请求前**调用。
///
/// 为什么不能一次生成、长期复用：飞牛音乐的 `authx` 签名带 nonce 与时间戳，
/// 静态头在长时间播放或多次 Range seek 后会被服务端判为过期。把生成时机放到
/// 每次请求，签名始终新鲜；`Err` 表示取令牌失败（冒泡成 HTTP 500）。
pub type HttpHeaderProvider =
    dyn Fn() -> Result<Vec<(String, String)>, String> + Send + Sync;

/// 构造「从 `offset` 起向上游发 HTTP Range 请求、流式写入 sink」的分段拉取器。
///
/// 与 [`SpoolReader`] 组合即得到一个随机 `RangeReader`：seek 落在当前分段之前时
/// [`SpoolReader`] 会从目标偏移重开一段，这里便重新发一次 HTTP 请求并**重签头**。
///
/// 适用对象是「HTTP 原生、但必须携带动态认证头」的来源（飞牛音乐、道理鱼）——
/// 它们无法像 WebDAV/S3/Plex 那样把 URL 直接交给播放引擎。
pub fn http_segment_opener(
    client: reqwest::blocking::Client,
    url: String,
    headers: Arc<HttpHeaderProvider>,
) -> Arc<SegmentOpener> {
    Arc::new(move |offset, sink, progress, abort| {
        let mut request = client.get(&url);
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        for (name, value) in headers()? {
            request = request.header(name, value);
        }

        let mut response = request.send().map_err(|e| format!("上游请求失败: {e}"))?;
        let status = response.status().as_u16();
        if status != 200 && status != 206 {
            return Err(format!("上游返回 HTTP {status}"));
        }

        let mut buf = vec![0u8; BRIDGE_CHUNK];
        let mut written = 0u64;
        loop {
            if abort.load(Ordering::SeqCst) {
                return Ok(());
            }
            let n = match response.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("读取上游失败: {e}")),
            };
            sink.write_all(&buf[..n])
                .map_err(|e| format!("写入桥接缓存失败: {e}"))?;
            written += n as u64;
            progress(written);
        }
    })
}

/// 解析 `Content-Range` 里的总长度（`bytes 0-1/12345` → `12345`）。
///
/// `*` 形式（总长未知）返回 `None`。纯函数，便于离线单测。
pub fn parse_content_range_total(raw: &str) -> Option<u64> {
    let total = raw.rsplit('/').next()?.trim();
    if total == "*" {
        return None;
    }
    total.parse::<u64>().ok()
}

/// 向上游发一次两字节 Range 探测，返回曲目总长度。
///
/// 桥接需要总长才能给出正确的 `Content-Length` 与 seek 边界；`Range: bytes=0-1`
/// 同时充当「认证头是否有效」的探针——401/403 会就此暴露，而不是等到播放中途。
pub fn probe_total_len(
    client: &reqwest::blocking::Client,
    url: &str,
    headers: &HttpHeaderProvider,
) -> Result<Option<u64>, String> {
    let mut request = client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-1");
    for (name, value) in headers()? {
        request = request.header(name, value);
    }

    let response = request.send().map_err(|e| format!("上游探测失败: {e}"))?;
    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err("上游拒绝认证（令牌无效或已过期）".to_string());
    }
    if status == 206 {
        if let Some(total) = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_range_total)
        {
            return Ok(Some(total));
        }
        return Ok(None);
    }
    if status == 200 {
        return Ok(response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok()));
    }
    Err(format!("上游探测返回 HTTP {status}"))
}

// ==================== 渐进式落盘读取器 ====================

/// 共享的下载进度状态。
struct SpoolState {
    /// 已可读的末尾绝对偏移（exclusive）。
    written: u64,
    /// 下载线程是否已结束（正常或出错）。
    done: bool,
    error: Option<String>,
}

/// 通用「渐进式落盘 + 随机读」读取器。
pub struct SpoolReader {
    opener: Arc<SegmentOpener>,
    total: Option<u64>,
    path: PathBuf,
    /// 当前分段在远端文件中的起始偏移（文件内偏移 0 对应它）。
    segment_start: u64,
    file: Option<std::fs::File>,
    state: Arc<(Mutex<SpoolState>, Condvar)>,
    thread: Option<thread::JoinHandle<()>>,
    abort: Option<Arc<AtomicBool>>,
}

impl SpoolReader {
    /// 创建并从 0 开始拉取（播放绝大多数从 0 开始）。
    pub fn new(total: Option<u64>, opener: Arc<SegmentOpener>) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "bayin-bridge-{:x}.spool",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut reader = SpoolReader {
            opener,
            total,
            path,
            segment_start: 0,
            file: None,
            state: Arc::new((
                Mutex::new(SpoolState {
                    written: 0,
                    done: false,
                    error: None,
                }),
                Condvar::new(),
            )),
            thread: None,
            abort: None,
        };
        reader.restart(0)?;
        Ok(reader)
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, SpoolState> {
        match self.state.0.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// 停掉当前分段，从 `from` 重新拉取。
    fn restart(&mut self, from: u64) -> Result<(), String> {
        // 1. 通知旧线程退出并等待它释放文件句柄。
        if let Some(abort) = self.abort.take() {
            abort.store(true, Ordering::SeqCst);
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        self.file = None;

        // 2. 重建临时文件（截断），并各持一个读写句柄。
        let write_handle = std::fs::File::create(&self.path)
            .map_err(|e| format!("创建桥接缓存文件失败: {e}"))?;
        let read_handle = std::fs::File::open(&self.path)
            .map_err(|e| format!("打开桥接缓存文件失败: {e}"))?;
        self.file = Some(read_handle);
        self.segment_start = from;
        {
            let mut s = self.lock_state();
            s.written = from;
            s.done = false;
            s.error = None;
        }

        // 3. 起新的下载线程。
        let abort = Arc::new(AtomicBool::new(false));
        self.abort = Some(abort.clone());
        let state = self.state.clone();
        let opener = self.opener.clone();
        let handle = thread::Builder::new()
            .name("bridge-spool".into())
            .spawn(move || {
                let mut sink = write_handle;
                let progress = |n: u64| {
                    let mut s = match state.0.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    s.written = from + n;
                    state.1.notify_all();
                };
                let result = opener(from, &mut sink, &progress, &abort);
                let mut s = match state.0.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                if let Err(e) = result {
                    s.error = Some(e);
                }
                s.done = true;
                state.1.notify_all();
            })
            .map_err(|e| format!("启动桥接下载线程失败: {e}"))?;
        self.thread = Some(handle);
        Ok(())
    }

    /// 等待 `offset` 变为可读；返回 `false` 表示已到末尾且无数据。
    fn wait_readable(&mut self, offset: u64) -> Result<bool, String> {
        loop {
            let (written, done, error) = {
                let s = self.lock_state();
                (s.written, s.done, s.error.clone())
            };

            if offset < written {
                return Ok(true);
            }
            if done {
                return match error {
                    Some(e) => Err(e),
                    None => Ok(false),
                };
            }

            let mut s = self.lock_state();
            while !s.done && s.written <= offset {
                s = match self.state.1.wait(s) {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
            }
        }
    }
}

impl RangeReader for SpoolReader {
    fn total_len(&self) -> Option<u64> {
        self.total
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, String> {
        if buf.is_empty() {
            return Ok(0);
        }

        // 落在当前分段之前 → 从目标偏移重新开一段（seek 不必先下载前半段）。
        if offset < self.segment_start {
            self.restart(offset)?;
        }

        if !self.wait_readable(offset)? {
            return Ok(0);
        }

        let (written, _) = {
            let s = self.lock_state();
            (s.written, ())
        };
        let available = (written - offset) as usize;
        let want = buf.len().min(available);

        let segment_start = self.segment_start;
        let file = self.file.as_mut().ok_or_else(|| "桥接缓存文件未就绪".to_string())?;
        file.seek(SeekFrom::Start(offset - segment_start))
            .map_err(|e| format!("桥接缓存定位失败: {e}"))?;

        let mut total = 0usize;
        while total < want {
            match file.read(&mut buf[total..want]) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("桥接缓存读取失败: {e}")),
            }
        }
        Ok(total)
    }
}

impl Drop for SpoolReader {
    fn drop(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.store(true, Ordering::SeqCst);
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        self.file = None;
        let _ = std::fs::remove_file(&self.path);
    }
}

// ==================== HTTP 请求解析 ====================

/// 解析出的 HTTP Range 请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteRange {
    /// 未携带 Range（整段返回）。
    None,
    /// 闭区间 `[start, end]`；`end` 为 `None` 表示到末尾。
    FromTo(u64, Option<u64>),
    /// 最后 N 字节。
    Suffix(u64),
    /// 语法错误 / 不可满足。
    Invalid,
}

/// 解析 `Range` 头（仅支持单区间 `bytes=` 形式）。
pub fn parse_range_header(raw: Option<&str>) -> ByteRange {
    let Some(raw) = raw else {
        return ByteRange::None;
    };
    let raw = raw.trim();
    let Some(spec) = raw.strip_prefix("bytes=") else {
        return ByteRange::Invalid;
    };
    // 多区间（含逗号）本实现不支持，直接当作无效，回退整段。
    if spec.contains(',') {
        return ByteRange::Invalid;
    }
    let spec = spec.trim();
    let Some((start_s, end_s)) = spec.split_once('-') else {
        return ByteRange::Invalid;
    };
    let (start_s, end_s) = (start_s.trim(), end_s.trim());

    if start_s.is_empty() {
        // bytes=-N：最后 N 字节
        return match end_s.parse::<u64>() {
            Ok(0) => ByteRange::Invalid,
            Ok(n) => ByteRange::Suffix(n),
            Err(_) => ByteRange::Invalid,
        };
    }

    let start = match start_s.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return ByteRange::Invalid,
    };
    if end_s.is_empty() {
        return ByteRange::FromTo(start, None);
    }
    match end_s.parse::<u64>() {
        Ok(end) if end >= start => ByteRange::FromTo(start, Some(end)),
        _ => ByteRange::Invalid,
    }
}

/// 解析出的请求行信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    /// 第一段路径（token），已去掉前导 `/`。
    pub token: String,
    pub range: ByteRange,
}

/// 解析请求行 + 头。返回 `None` 表示请求不完整或不是 GET。
pub fn parse_request(head: &str) -> Option<HttpRequest> {
    let mut lines = head.lines();
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }

    // /<token>/<name> → token
    let token = target
        .trim_start_matches('/')
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .to_string();

    let mut range = ByteRange::None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("range") {
                range = parse_range_header(Some(v.trim()));
            }
        }
    }

    Some(HttpRequest {
        method,
        token,
        range,
    })
}

// ==================== 响应写出（可单测的纯 IO） ====================

/// 依据请求把 `reader` 的数据写成 HTTP 响应。
///
/// 抽成独立函数是为了能用内存 sink 做**离线**单测：不依赖真实 socket。
pub fn write_response(out: &mut dyn Write, req: &HttpRequest, reader: &mut dyn RangeReader) -> std::io::Result<()> {
    let total = reader.total_len();

    // 计算实际区间。
    let (status, start, end_inclusive) = match req.range {
        ByteRange::None => (200u16, 0u64, total.map(|t| t.saturating_sub(1))),
        ByteRange::FromTo(start, end) => {
            if let Some(t) = total {
                if start >= t {
                    return write_error(out, 416, "Requested Range Not Satisfiable", Some(t));
                }
                let end = end.map(|e| e.min(t.saturating_sub(1))).unwrap_or(t.saturating_sub(1));
                (206, start, Some(end))
            } else {
                (206, start, end)
            }
        }
        ByteRange::Suffix(n) => match total {
            Some(t) if t > 0 => {
                let start = t.saturating_sub(n);
                (206, start, Some(t.saturating_sub(1)))
            }
            _ => return write_error(out, 416, "Requested Range Not Satisfiable", total),
        },
        // 语法无效：按 HTTP 语义可忽略该头，整体返回。
        ByteRange::Invalid => (200u16, 0u64, total.map(|t| t.saturating_sub(1))),
    };

    let body_len = match end_inclusive {
        Some(end) if end >= start => end - start + 1,
        Some(_) => 0,
        None => 0, // 未知长度：靠连接关闭定界
    };
    let length_known = end_inclusive.is_some();

    let mut head = String::new();
    head.push_str(match status {
        206 => "HTTP/1.1 206 Partial Content\r\n",
        416 => "HTTP/1.1 416 Requested Range Not Satisfiable\r\n",
        _ => "HTTP/1.1 200 OK\r\n",
    });
    head.push_str("Content-Type: application/octet-stream\r\n");
    head.push_str("Accept-Ranges: bytes\r\n");
    if status == 206 {
        if let Some(t) = total {
            head.push_str(&format!(
                "Content-Range: bytes {}-{}/{}\r\n",
                start,
                end_inclusive.unwrap_or(t.saturating_sub(1)),
                t
            ));
        }
    }
    if length_known {
        head.push_str(&format!("Content-Length: {body_len}\r\n"));
    }
    head.push_str("Cache-Control: no-store\r\n");
    // 不使用 chunked：未知长度时靠 `Connection: close` 定界。
    head.push_str("Connection: close\r\n\r\n");
    out.write_all(head.as_bytes())?;

    if req.method == "HEAD" {
        return out.flush();
    }

    let mut offset = start;
    let mut remaining = if length_known { body_len } else { u64::MAX };
    let mut buf = vec![0u8; BRIDGE_CHUNK];
    while remaining > 0 {
        let want = buf.len().min(remaining.min(usize::MAX as u64) as usize);
        let n = match reader.read_at(offset, &mut buf[..want]) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                // 头已发出，无法改状态码；中断连接让客户端感知失败。
                log::warn!("[bridge] 读取失败: {e}");
                break;
            }
        };
        out.write_all(&buf[..n])?;
        offset += n as u64;
        remaining -= n as u64;
    }
    out.write_all(&[])?;
    out.flush()
}

fn write_error(out: &mut dyn Write, status: u16, reason: &str, total: Option<u64>) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nAccept-Ranges: bytes\r\n");
    if status == 416 {
        if let Some(t) = total {
            head.push_str(&format!("Content-Range: bytes */{t}\r\n"));
        }
    }
    head.push_str("Connection: close\r\n\r\n");
    out.write_all(head.as_bytes())?;
    out.flush()
}

// ==================== 注册表与服务 ====================

struct Entry {
    config: StreamServerConfig,
    song_id: String,
    reader: Option<Arc<Mutex<Box<dyn RangeReader>>>>,
    last_used: Instant,
}

fn reader_table() -> &'static Mutex<HashMap<String, Entry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_registry() -> std::sync::MutexGuard<'static, HashMap<String, Entry>> {
    match reader_table().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// 由「来源身份 + 曲目」得到稳定的桥接 token。
///
/// 同一首歌重复登记必须得到同一 URL：播放器在 seek 时会对同一 URL 再次发起
/// Range 请求，token 变化会导致缓存/分段全部作废。
pub fn token_for(config: &StreamServerConfig, song_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config.server_type.as_str().as_bytes());
    hasher.update(b"\x1f");
    hasher.update(config.server_url.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(config.username.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(config.base_path.as_deref().unwrap_or("").as_bytes());
    hasher.update(b"\x1f");
    hasher.update(song_id.as_bytes());
    let digest = hasher.finalize();
    format!("{:x}", digest)[..32].to_string()
}

/// 把曲目名清洗成可安全放进 URL 路径、且保留扩展名的形式。
///
/// 保留扩展名是为了让 `AudioDecoder` 能通过 URL 后缀给出 symphonia 探测提示。
pub fn safe_object_name(song_id: &str) -> String {
    let base = song_id
        .split(['?', '#'])
        .next()
        .unwrap_or(song_id)
        .rsplit('/')
        .next()
        .unwrap_or("audio");

    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) if !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()) => {
            (s, e.to_ascii_lowercase())
        }
        _ => (base, "bin".to_string()),
    };

    let mut clean: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if clean.len() > 48 {
        clean.truncate(48);
    }
    if clean.is_empty() {
        clean = "audio".to_string();
    }
    format!("{clean}.{ext}")
}

/// 惰性启动回环桥接服务，返回监听端口。
pub fn ensure_started() -> Result<u16, String> {
    static PORT: OnceLock<Result<u16, String>> = OnceLock::new();
    PORT.get_or_init(|| {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| format!("启动本地桥接服务失败: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("读取桥接端口失败: {e}"))?
            .port();
        thread::Builder::new()
            .name("bridge-http".into())
            .spawn(move || accept_loop(listener))
            .map_err(|e| format!("启动桥接监听线程失败: {e}"))?;
        log::info!("[bridge] 本地 HTTP+Range 桥接已监听 127.0.0.1:{port}");
        Ok(port)
    })
    .clone()
}

/// 登记一个远端曲目，返回可供播放器使用的本地 HTTP URL。
pub fn register(config: &StreamServerConfig, song_id: &str) -> Result<String, String> {
    let port = ensure_started()?;
    let token = token_for(config, song_id);
    {
        let mut reg = lock_registry();
        if let Some(entry) = reg.get_mut(&token) {
            entry.last_used = Instant::now();
        } else {
            // 淘汰最久未用的一项，避免长会话无限增长。
            if reg.len() >= MAX_CACHED_READERS {
                if let Some(oldest) = reg
                    .iter()
                    .min_by_key(|(_, e)| e.last_used)
                    .map(|(k, _)| k.clone())
                {
                    reg.remove(&oldest);
                }
            }
            reg.insert(
                token.clone(),
                Entry {
                    config: config.clone(),
                    song_id: song_id.to_string(),
                    reader: None,
                    last_used: Instant::now(),
                },
            );
        }
    }
    Ok(format!(
        "http://127.0.0.1:{port}/{token}/{}",
        safe_object_name(song_id)
    ))
}

/// 取（必要时创建）某 token 对应的读取器。
fn reader_for(token: &str) -> Option<Arc<Mutex<Box<dyn RangeReader>>>> {
    // 先看是否已有读取器（避免每次请求都重新建连接 / 重新开分段）。
    {
        let mut reg = lock_registry();
        if let Some(entry) = reg.get_mut(token) {
            entry.last_used = Instant::now();
            if let Some(reader) = &entry.reader {
                return Some(reader.clone());
            }
        } else {
            return None;
        }
    }

    // 创建读取器：需要拿到连接器，因此不能持有注册表锁（连接器解析可能耗时）。
    let (config, song_id) = {
        let reg = lock_registry();
        let entry = reg.get(token)?;
        (entry.config.clone(), entry.song_id.clone())
    };

    let reg = crate::source::registry();
    let connector = match reg.resolve(&config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[bridge] 无可用连接器: {e}");
            return None;
        }
    };
    let reader = match connector.open_reader(&config, &song_id) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[bridge] 打开读取器失败: {e}");
            return None;
        }
    };
    let boxed: Arc<Mutex<Box<dyn RangeReader>>> = Arc::new(Mutex::new(reader));

    let mut reg = lock_registry();
    let entry = reg.get_mut(token)?;
    // 竞态：并发两个请求都创建了读取器时，保留先写入的那个。
    if let Some(existing) = &entry.reader {
        return Some(existing.clone());
    }
    entry.reader = Some(boxed.clone());
    Some(boxed)
}

fn accept_loop(listener: TcpListener) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::Builder::new()
                    .name("bridge-conn".into())
                    .spawn(move || {
                        if let Err(e) = handle_connection(stream) {
                            log::debug!("[bridge] 连接处理结束: {e}");
                        }
                    })
                    .ok();
            }
            Err(e) => log::debug!("[bridge] accept 失败: {e}"),
        }
    }
}

fn handle_connection(stream: TcpStream) -> std::io::Result<()> {
    let _ = stream.set_nodelay(true);
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(e) => return Err(e),
    });

    // 读请求行 + 头（直到空行）。
    let mut head = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
        if head.len() > 16 * 1024 {
            break;
        }
    }

    let mut out = stream;
    let Some(req) = parse_request(&head) else {
        return write_error(&mut out, 400, "Bad Request", None);
    };

    let Some(shared) = reader_for(&req.token) else {
        return write_error(&mut out, 404, "Not Found", None);
    };

    let mut guard = match shared.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let result = write_response(&mut out, &req, guard.as_mut());
    drop(guard);
    // 显式关闭写端，让客户端立即感知 EOF（未知长度时依赖它定界）。
    let _ = out.shutdown(std::net::Shutdown::Write);
    result
}

/// 仅测试用：直接登记一个读取器，绕过「配置 → 连接器 → 读取器」的解析链。
#[cfg(test)]
fn register_direct(config: &StreamServerConfig, song_id: &str, reader: Box<dyn RangeReader>) -> String {
    let port = ensure_started().expect("桥接服务应能启动");
    let token = token_for(config, song_id);
    {
        let mut reg = lock_registry();
        reg.insert(
            token.clone(),
            Entry {
                config: config.clone(),
                song_id: song_id.to_string(),
                reader: Some(Arc::new(Mutex::new(reader))),
                last_used: Instant::now(),
            },
        );
    }
    format!(
        "http://127.0.0.1:{port}/{token}/{}",
        safe_object_name(song_id)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// 用内存 Vec 模拟远端文件，验证整个 HTTP 语义（离线、确定性）。
    struct VecReader {
        data: Vec<u8>,
        reads: Arc<AtomicUsize>,
    }

    impl VecReader {
        fn new(data: Vec<u8>) -> Self {
            VecReader {
                data,
                reads: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl RangeReader for VecReader {
        fn total_len(&self) -> Option<u64> {
            Some(self.data.len() as u64)
        }
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let offset = offset as usize;
            if offset >= self.data.len() {
                return Ok(0);
            }
            let n = buf.len().min(self.data.len() - offset);
            buf[..n].copy_from_slice(&self.data[offset..offset + n]);
            Ok(n)
        }
    }

    fn req(range: ByteRange) -> HttpRequest {
        HttpRequest {
            method: "GET".to_string(),
            token: "t".to_string(),
            range,
        }
    }

    fn do_get(reader: &mut dyn RangeReader, range: ByteRange) -> String {
        let mut sink: Vec<u8> = Vec::new();
        write_response(&mut sink, &req(range), reader).unwrap();
        String::from_utf8_lossy(&sink).to_string()
    }

    fn body_of(raw: &str) -> &[u8] {
        let idx = raw.find("\r\n\r\n").expect("响应缺少空行") + 4;
        // 头部是 ASCII，body 按字节切分。
        let head_len = raw[..idx].len();
        &raw.as_bytes()[head_len..]
    }

    #[test]
    fn full_get_returns_200_with_content_length() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let raw = do_get(&mut r, ByteRange::None);
        assert!(raw.starts_with("HTTP/1.1 200 OK\r\n"), "{raw}");
        assert!(raw.contains("Content-Length: 10\r\n"));
        assert!(raw.contains("Accept-Ranges: bytes\r\n"));
        assert!(!raw.contains("Content-Range"));
        assert_eq!(body_of(&raw), b"0123456789");
    }

    #[test]
    fn open_ended_range_returns_206_with_correct_content_range() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let raw = do_get(&mut r, ByteRange::FromTo(4, None));
        assert!(raw.starts_with("HTTP/1.1 206 Partial Content\r\n"), "{raw}");
        assert!(raw.contains("Content-Range: bytes 4-9/10\r\n"), "{raw}");
        assert!(raw.contains("Content-Length: 6\r\n"));
        assert_eq!(body_of(&raw), b"456789");
    }

    #[test]
    fn bounded_range_is_clamped_to_file_end() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let raw = do_get(&mut r, ByteRange::FromTo(8, Some(99)));
        assert!(raw.contains("Content-Range: bytes 8-9/10\r\n"), "{raw}");
        assert_eq!(body_of(&raw), b"89");
    }

    #[test]
    fn suffix_range_returns_tail() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let raw = do_get(&mut r, ByteRange::Suffix(3));
        assert!(raw.contains("Content-Range: bytes 7-9/10\r\n"), "{raw}");
        assert_eq!(body_of(&raw), b"789");
    }

    #[test]
    fn unsatisfiable_range_returns_416() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let raw = do_get(&mut r, ByteRange::FromTo(20, None));
        assert!(raw.starts_with("HTTP/1.1 416 "), "{raw}");
        assert!(raw.contains("Content-Range: bytes */10\r\n"), "{raw}");
        assert!(body_of(&raw).is_empty());
    }

    #[test]
    fn range_starting_at_eof_is_unsatisfiable() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let raw = do_get(&mut r, ByteRange::FromTo(10, None));
        assert!(raw.starts_with("HTTP/1.1 416 "), "{raw}");
    }

    /// 关键回归点：播放器 seek 会先读尾部做 `SeekFrom::End`，此时必须是 206 且
    /// 长度可判定——否则 symphonia 会认为流不可 seek。
    #[test]
    fn seek_style_range_is_reported_as_seekable() {
        let mut r = VecReader::new(vec![7u8; 1_000_000]);
        let raw = do_get(&mut r, ByteRange::FromTo(999_000, None));
        assert!(raw.contains("HTTP/1.1 206"));
        assert!(raw.contains("Content-Range: bytes 999000-999999/1000000\r\n"));
        assert!(raw.contains("Content-Length: 1000\r\n"));
    }

    #[test]
    fn invalid_range_falls_back_to_full_response() {
        let mut r = VecReader::new(b"abcdef".to_vec());
        let raw = do_get(&mut r, ByteRange::Invalid);
        assert!(raw.starts_with("HTTP/1.1 200 OK\r\n"), "{raw}");
        assert_eq!(body_of(&raw), b"abcdef");
    }

    #[test]
    fn head_request_has_headers_but_no_body() {
        let mut r = VecReader::new(b"0123456789".to_vec());
        let mut sink: Vec<u8> = Vec::new();
        write_response(
            &mut sink,
            &HttpRequest {
                method: "HEAD".to_string(),
                token: "t".to_string(),
                range: ByteRange::None,
            },
            &mut r,
        )
        .unwrap();
        let raw = String::from_utf8_lossy(&sink).to_string();
        assert!(raw.contains("Content-Length: 10\r\n"));
        assert!(body_of(&raw).is_empty(), "HEAD 不应返回 body");
        assert_eq!(r.reads.load(Ordering::SeqCst), 0, "HEAD 不应读取数据");
    }

    #[test]
    fn range_header_parsing() {
        assert_eq!(parse_range_header(None), ByteRange::None);
        assert_eq!(parse_range_header(Some("bytes=0-")), ByteRange::FromTo(0, None));
        assert_eq!(parse_range_header(Some("bytes=100-199")), ByteRange::FromTo(100, Some(199)));
        assert_eq!(parse_range_header(Some("bytes=-500")), ByteRange::Suffix(500));
        assert_eq!(parse_range_header(Some("  bytes= 42 -  ")), ByteRange::FromTo(42, None));
        // 非法输入一律 Invalid（回退整段），不应 panic。
        assert_eq!(parse_range_header(Some("items=0-")), ByteRange::Invalid);
        assert_eq!(parse_range_header(Some("bytes=abc-")), ByteRange::Invalid);
        assert_eq!(parse_range_header(Some("bytes=5-1")), ByteRange::Invalid);
        assert_eq!(parse_range_header(Some("bytes=-0")), ByteRange::Invalid);
        assert_eq!(parse_range_header(Some("bytes=0-10,20-30")), ByteRange::Invalid);
        assert_eq!(parse_range_header(Some("bytes=")), ByteRange::Invalid);
    }

    #[test]
    fn request_parsing_extracts_token_and_range() {
        let head = "GET /abc123/song.flac HTTP/1.1\r\nHost: 127.0.0.1:5000\r\nRange: bytes=1024-\r\n\r\n";
        let r = parse_request(head).unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.token, "abc123");
        assert_eq!(r.range, ByteRange::FromTo(1024, None));

        // 无 Range。
        let r2 = parse_request("GET /tok HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        assert_eq!(r2.token, "tok");
        assert_eq!(r2.range, ByteRange::None);

        // 非 GET/HEAD（如 POST）不接受。
        assert!(parse_request("POST /x HTTP/1.1\r\n\r\n").is_none());
        assert!(parse_request("").is_none());
    }

    #[test]
    fn token_is_stable_and_config_sensitive() {
        use crate::models::ServerType;
        let base = StreamServerConfig {
            server_type: ServerType::Ftp,
            server_name: "t".into(),
            server_url: "ftp://nas:21".into(),
            username: "u".into(),
            password: "p".into(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        };
        // 同一首歌重复取 token 必须一致（seek 时 URL 不能变）。
        assert_eq!(token_for(&base, "/a/b.flac"), token_for(&base, "/a/b.flac"));
        // 不同歌 / 不同服务器必须不同。
        assert_ne!(token_for(&base, "/a/b.flac"), token_for(&base, "/a/c.flac"));
        let mut other = base.clone();
        other.server_url = "ftp://other:21".into();
        assert_ne!(token_for(&base, "/a/b.flac"), token_for(&other, "/a/b.flac"));
        assert_eq!(token_for(&base, "/a/b.flac").len(), 32);
    }

    #[test]
    fn safe_object_name_keeps_extension_for_probing_hint() {
        assert_eq!(safe_object_name("/music/a/Song.flac"), "Song.flac");
        assert_eq!(safe_object_name("http://n/a/song.MP3?x=1"), "song.mp3");
        // 非 ASCII / 特殊字符被清洗，但仍保留扩展名。
        assert_eq!(safe_object_name("/音乐/歌 名.mp3"), "___.mp3");
        // 无扩展名 → 兜底 .bin，且不产生路径分隔符。
        assert_eq!(safe_object_name("/music/album/song"), "song.bin");
        assert!(!safe_object_name("/a/../../etc/passwd").contains('/'));
    }

    #[test]
    fn unknown_token_yields_no_reader() {
        assert!(reader_for("definitely-not-registered").is_none());
    }

    /// SpoolReader：渐进可用于顺序读，且能从中途重新开段。
    #[test]
    fn spool_reader_serves_progressively_and_can_jump() {
        let source: Arc<Vec<u8>> = Arc::new((0..=255u8).cycle().take(512 * 1024).collect());
        let opener = {
            let source = source.clone();
            Arc::new(move |offset: u64, sink: &mut dyn Write, progress: &dyn Fn(u64), abort: &AtomicBool| {
                let mut written = 0u64;
                let mut pos = offset as usize;
                while pos < source.len() {
                    if abort.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    let end = (pos + 4096).min(source.len());
                    sink.write_all(&source[pos..end]).map_err(|e| e.to_string())?;
                    written += (end - pos) as u64;
                    pos = end;
                    progress(written);
                }
                Ok(())
            }) as Arc<SegmentOpener>
        };

        let mut reader = SpoolReader::new(Some(source.len() as u64), opener).unwrap();
        assert_eq!(reader.total_len(), Some(source.len() as u64));

        // 从 0 顺序读。
        let mut buf = vec![0u8; 8192];
        let n = reader.read_at(0, &mut buf).unwrap();
        assert!(n > 0, "首段读取应有数据");
        assert_eq!(&buf[..n], &source[..n]);

        // 向前跳（模拟 seek）：必须能直接从该偏移开始读，而不是先补下载前半段。
        let jump = 300 * 1024u64;
        let n2 = reader.read_at(jump, &mut buf).unwrap();
        assert!(n2 > 0, "跳转后应有数据");
        assert_eq!(&buf[..n2], &source[jump as usize..jump as usize + n2]);

        // 越界读取 → EOF。
        assert_eq!(reader.read_at(source.len() as u64 + 10, &mut buf).unwrap(), 0);
    }

    /// 下载出错时必须把错误冒泡给读者，而不是静默返回 EOF。
    #[test]
    fn spool_reader_surfaces_opener_error() {
        let opener = Arc::new(|_o: u64, _s: &mut dyn Write, _p: &dyn Fn(u64), _a: &AtomicBool| {
            Err("远端连接被拒绝".to_string())
        }) as Arc<SegmentOpener>;
        let mut reader = SpoolReader::new(Some(100), opener).unwrap();
        let mut buf = vec![0u8; 16];
        let err = reader.read_at(0, &mut buf).unwrap_err();
        assert!(err.contains("远端连接被拒绝"), "{err}");
    }

    // ---------- 走真实 socket 的端到端验证 ----------

    use crate::models::ServerType;
    use std::net::TcpStream;
    use std::time::Duration;

    fn test_config() -> StreamServerConfig {
        StreamServerConfig {
            server_type: ServerType::Ftp,
            server_name: "t".into(),
            server_url: "ftp://nas:21".into(),
            username: "u".into(),
            password: "p".into(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: None,
            access_token: None,
            user_id: None,
        }
    }

    /// 发一个原始 HTTP/1.1 请求并读回完整响应。
    fn raw_http(url: &str, extra_headers: &str) -> Vec<u8> {
        let rest = url.strip_prefix("http://").expect("仅支持 http://");
        let (authority, path) = rest.split_once('/').expect("URL 应含路径");
        let mut stream = TcpStream::connect(authority).expect("应能连上本地桥接端口");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let request = format!(
            "GET /{path} HTTP/1.1\r\nHost: {authority}\r\n{extra_headers}Connection: close\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();
        let mut out = Vec::new();
        stream.read_to_end(&mut out).unwrap();
        out
    }

    fn split_http(raw: &[u8]) -> (String, Vec<u8>) {
        let pos = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("响应缺少头体分隔");
        let head = String::from_utf8_lossy(&raw[..pos]).to_string();
        (head, raw[pos + 4..].to_vec())
    }

    /// 真实监听 + 真实 TCP 的完整链路：登记 → GET → Range → 校验字节。
    #[test]
    fn end_to_end_range_over_real_socket() {
        let data: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
        let cfg = test_config();
        let url = register_direct(&cfg, "album/song.flac", Box::new(VecReader::new(data.clone())));
        assert!(url.starts_with("http://127.0.0.1:"), "{url}");
        assert!(url.ends_with("/song.flac"), "{url}");

        // 1. 无 Range → 200 + 完整 body（播放器打开流时就是这么做的）。
        let (head, body) = split_http(&raw_http(&url, ""));
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert!(head.contains("Content-Length: 300000"), "{head}");
        assert_eq!(body.len(), 300_000);
        assert_eq!(&body[..16], &data[..16]);

        // 2. Range → 206 + 精确切片（播放器 seek 时这么做）。
        let (head, body) = split_http(&raw_http(&url, "Range: bytes=200000-200063\r\n"));
        assert!(head.starts_with("HTTP/1.1 206 Partial Content"), "{head}");
        assert!(
            head.contains("Content-Range: bytes 200000-200063/300000"),
            "{head}"
        );
        assert_eq!(body, &data[200_000..=200_063]);

        // 3. 开放区间 → 到文件末尾。
        let (head, body) = split_http(&raw_http(&url, "Range: bytes=299990-\r\n"));
        assert!(head.contains("Content-Range: bytes 299990-299999/300000"), "{head}");
        assert_eq!(body, &data[299_990..]);

        // 4. 越界 Range → 416。
        let (head, body) = split_http(&raw_http(&url, "Range: bytes=999999-\r\n"));
        assert!(head.starts_with("HTTP/1.1 416"), "{head}");
        assert!(body.is_empty());
    }

    /// 未知 token 必须 404（而不是挂起或返回空 200）。
    #[test]
    fn end_to_end_unknown_token_is_404() {
        let port = ensure_started().unwrap();
        let (head, _) = split_http(&raw_http(
            &format!("http://127.0.0.1:{port}/deadbeef/song.mp3"),
            "",
        ));
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    }

    /// 非 GET/非法请求行不得 panic，应返回 400。
    #[test]
    fn end_to_end_bad_request_is_400() {
        let port = ensure_started().unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream.write_all(b"BREW /coffee HTTP/1.1\r\n\r\n").unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let (head, _) = split_http(&raw);
        assert!(head.starts_with("HTTP/1.1 400"), "{head}");
    }
}
