//! FTP 音乐源（阶段 B4）。
//!
//! ## 为什么 FTP 需要本地桥接
//!
//! FTP 不是 HTTP：它没有 Range 语义（只有 `REST` 断点续传）、没有 `Content-Length`
//! 响应头，播放引擎完全无法直接消费。因此本模块把远端文件包装成
//! [`crate::source::bridge::RangeReader`]，由 [`crate::source::bridge`] 在
//! `127.0.0.1` 上暴露成支持 Range 的 HTTP 流。
//!
//! 读取策略是**按需分段下载**而非全量预下载：`SpoolReader` 从请求偏移开始顺序拉取
//! 写入本地临时文件，向前 seek 时直接从新偏移另开一段（配合 FTP `REST`），所以
//! 跳到 80% 处不需要先下载前 80%。
//!
//! ## 协议实现
//!
//! 只有明文 FTP（`suppaftp` 默认 feature）。FTPS 需要引入 TLS 后端（`native-tls`
//! 会把构建绑到系统 OpenSSL/SChannel，影响 Android/交叉编译），暂不引入。
//!
//! `StreamServerConfig` 字段映射：
//! | 字段 | 含义 |
//! |---|---|
//! | `server_url` | `ftp://host:port` / `host:port` / `host`（默认 21） |
//! | `username` / `password` | 登录凭据（匿名登录可留空） |
//! | `base_path`  | 登录后的起始目录（可选，如 `/music`） |

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use suppaftp::types::FileType;
use suppaftp::FtpStream;

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::cover::CoverCache;

/// 元数据探测读取的字节数。
const METADATA_PROBE_BYTES: usize = 1024 * 1024;
/// 单个列表项的解析结果。
#[derive(Debug, Clone, PartialEq)]
pub struct FtpEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
}

// ==================== 端点与连接 ====================

/// 解析 FTP 端点：支持 `ftp://host:port`、`host:port`、`host`。
pub fn parse_endpoint(server_url: &str) -> Result<(String, u16), String> {
    let raw = server_url.trim();
    if raw.is_empty() {
        return Err("未填写 FTP 地址".to_string());
    }

    // 带 scheme 时交给 url 解析（可顺带处理 `ftp://user@host`）。
    if let Ok(parsed) = url::Url::parse(raw) {
        if parsed.scheme().starts_with("ftp") {
            let host = parsed
                .host_str()
                .ok_or_else(|| "FTP 地址缺少主机名".to_string())?
                .to_string();
            let port = parsed.port().unwrap_or(21);
            return Ok((host, port));
        }
    }

    // 无 scheme：手工剥离路径与端口。
    let without_scheme = raw.split("://").nth(1).unwrap_or(raw);
    let authority = without_scheme.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err(format!("无法解析 FTP 地址: {raw}"));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => {
            let port = port
                .parse::<u16>()
                .map_err(|_| format!("FTP 端口非法: {port}"))?;
            Ok((host.to_string(), port))
        }
        _ => Ok((authority.to_string(), 21)),
    }
}

/// 登录后的起始目录（规范化，无首尾多余斜杠）。
fn base_dir(config: &StreamServerConfig) -> String {
    config
        .base_path
        .as_deref()
        .map(str::trim)
        .map(|s| s.trim_matches('/').to_string())
        .unwrap_or_default()
}

/// 建立一次登录会话（已切到二进制模式并 `cwd` 到起始目录）。
fn connect(config: &StreamServerConfig) -> Result<FtpStream, String> {
    let (host, port) = parse_endpoint(&config.server_url)?;
    let mut ftp = FtpStream::connect((host.as_str(), port))
        .map_err(|e| format!("连接 FTP {host}:{port} 失败: {e}"))?;

    let user = if config.username.is_empty() {
        "anonymous"
    } else {
        config.username.as_str()
    };
    ftp.login(user, config.password.as_str())
        .map_err(|e| format!("FTP 登录失败: {e}"))?;

    // 音频是二进制；不设置会让服务器做 CRLF 转换，直接损坏文件。
    ftp.transfer_type(FileType::Binary)
        .map_err(|e| format!("FTP 切换二进制模式失败: {e}"))?;

    let base = base_dir(config);
    if !base.is_empty() {
        ftp.cwd(&base)
            .map_err(|e| format!("FTP 进入目录 {base} 失败: {e}"))?;
    }
    Ok(ftp)
}

/// 规范化相对路径（统一分隔符，去掉首尾斜杠）。
fn normalize_rel(path: &str) -> String {
    path.trim().replace('\\', "/").trim_matches('/').to_string()
}

/// 拼接相对路径（两侧都做规范化，避免出现 `a//b`）。
pub fn join_rel(dir: &str, name: &str) -> String {
    let dir = normalize_rel(dir);
    let name = name.trim().replace('\\', "/");
    let name = name.trim_matches('/');
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

// ==================== 列表解析（纯函数，可离线单测） ====================

/// 解析一行 MLSD 输出：`type=file;size=123;modify=20240101120000; name.flac`
pub fn parse_mlsd_line(line: &str) -> Option<FtpEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() {
        return None;
    }
    let (facts, name) = line.split_once(' ')?;
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    let mut is_dir = false;
    let mut size = None;
    let mut modified = None;
    for fact in facts.split(';') {
        let Some((k, v)) = fact.split_once('=') else {
            continue;
        };
        match k.trim().to_ascii_lowercase().as_str() {
            "type" => {
                let v = v.trim();
                is_dir = v.eq_ignore_ascii_case("dir")
                    || v.eq_ignore_ascii_case("cdir")
                    || v.eq_ignore_ascii_case("pdir")
            }
            "size" => size = v.trim().parse::<u64>().ok(),
            "modify" => modified = parse_ftp_time(v.trim()),
            _ => {}
        }
    }
    Some(FtpEntry {
        name: name.to_string(),
        is_dir,
        size,
        modified,
    })
}

/// 解析 MLSD 的 `modify` 时间：`YYYYMMDDHHMMSS[.sss]`（UTC）。
pub fn parse_ftp_time(v: &str) -> Option<i64> {
    let head = v.split('.').next()?;
    if head.len() < 14 || !head[..14].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let dt = chrono::NaiveDateTime::parse_from_str(&head[..14], "%Y%m%d%H%M%S").ok()?;
    Some(dt.and_utc().timestamp())
}

/// 解析一行 Unix 风格 `LIST` 输出：
/// `-rw-r--r-- 1 user group 12345 Jan 10 12:34 song.flac`
///
/// 回退解析器：部分服务器不支持 MLSD，只能靠 `LIST`。文件名可能含空格，因此按
/// "第 9 个字段的起始字节位置"切分，而不是简单 `split_whitespace` 后拼回。
pub fn parse_unix_list_line(line: &str) -> Option<FtpEntry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() || line.len() < 10 {
        return None;
    }

    let mut starts: Vec<usize> = Vec::new();
    let mut in_field = false;
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            in_field = false;
        } else if !in_field {
            starts.push(idx);
            in_field = true;
        }
    }
    if starts.len() < 9 {
        return None;
    }

    let perms = &line[starts[0]..];
    let first = perms.chars().next()?;
    // 只接受权限串（`-` 文件 / `d` 目录），避免把其它输出误判成列表。
    if first != '-' && first != 'd' && first != 'l' {
        return None;
    }

    let size = line[starts[4]..starts[5]].trim().parse::<u64>().ok();
    let name = line[starts[8]..].trim().to_string();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    Some(FtpEntry {
        name,
        // 符号链接按文件处理（音乐库里的链接一般指向文件）。
        is_dir: first == 'd',
        size,
        modified: None,
    })
}

// ==================== 目录列举 ====================

/// 列出某目录下的条目（MLSD 优先，失败回退 LIST）。
fn list_entries(ftp: &mut FtpStream, dir: &str) -> Result<Vec<FtpEntry>, String> {
    let target = normalize_rel(dir);
    let arg = if target.is_empty() { None } else { Some(target.as_str()) };

    match ftp.mlsd(arg) {
        Ok(lines) => {
            let parsed: Vec<FtpEntry> = lines.iter().filter_map(|l| parse_mlsd_line(l)).collect();
            if !parsed.is_empty() {
                return Ok(parsed);
            }
            // MLSD 返回空：可能是空目录，也可能服务器不认——回退 LIST 确认。
        }
        Err(_) => {}
    }

    let lines = ftp
        .list(arg)
        .map_err(|e| format!("FTP 列目录失败: {e}"))?;
    Ok(lines.iter().filter_map(|l| parse_unix_list_line(l)).collect())
}

/// 解析配置对应的根目录 URL（相对路径形式，空串表示登录目录）。
pub fn root_dir_url(_config: &StreamServerConfig) -> String {
    String::new()
}

/// 浏览目录（`dir_url` 为空取配置起始目录）。
pub fn list_dir(
    config: &StreamServerConfig,
    dir_url: Option<&str>,
) -> Result<Vec<crate::source::DirEntry>, String> {
    let mut ftp = connect(config)?;
    let dir = dir_url.map(normalize_rel).unwrap_or_default();
    let entries = list_entries(&mut ftp, &dir)?;
    let _ = ftp.quit();

    Ok(entries
        .into_iter()
        .map(|e| crate::source::DirEntry {
            name: e.name.clone(),
            url: join_rel(&dir, &e.name),
            is_dir: e.is_dir,
            size: if e.is_dir { None } else { e.size },
            modified: e.modified,
        })
        .collect())
}

/// 测试连接：登录 + 列一次根目录。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    match connect(config) {
        Ok(mut ftp) => {
            let result = list_entries(&mut ftp, "");
            let _ = ftp.quit();
            match result {
                Ok(entries) => ConnectionTestResult {
                    success: true,
                    message: format!("已连接 FTP，当前目录 {} 项", entries.len()),
                    server_version: None,
                },
                Err(e) => ConnectionTestResult {
                    success: false,
                    message: e,
                    server_version: None,
                },
            }
        }
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

// ==================== 扫描 ====================

/// 递归收集目录下的音频条目（返回 (相对路径, 大小, 修改时间)）。
fn walk_audio(
    ftp: &mut FtpStream,
    dir: &str,
    max_entries: usize,
) -> Result<Vec<(String, Option<u64>, Option<i64>)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![normalize_rel(dir)];
    let mut visited = 0usize;

    while let Some(current) = stack.pop() {
        let entries = list_entries(ftp, &current)?;
        for e in entries {
            visited += 1;
            if visited > max_entries {
                log::warn!("[FTP] 目录项超过 {max_entries}，提前停止递归");
                return Ok(out);
            }
            let rel = join_rel(&current, &e.name);
            if e.is_dir {
                stack.push(rel);
            } else if crate::utils::localfs::is_audio_file(Path::new(&e.name)) {
                out.push((rel, e.size, e.modified));
            }
        }
    }
    Ok(out)
}

/// 扫描目录（`dir_url` 为空取配置根目录）。
pub fn scan_dir(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    dir_url: &str,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let mut ftp = connect(config)?;
    let files = walk_audio(&mut ftp, dir_url, 100_000)?;
    let total = files.len();
    let mut out = Vec::with_capacity(total);

    for (idx, (rel, size, modified)) in files.into_iter().enumerate() {
        let unchanged = match (existing_modified, modified) {
            (Some(map), Some(m)) => map.get(&rel).map(|old| *old == m).unwrap_or(false),
            _ => false,
        };

        let mut song = if unchanged || !read_tags {
            crate::utils::webdav::fallback_song_with_modified(&rel, modified)
        } else {
            // 复用已建立的会话做头部探测，避免逐首重连。
            match probe_with(&mut ftp, &rel, cover_cache) {
                Some(s) => s,
                None => crate::utils::webdav::fallback_song_with_modified(&rel, modified),
            }
        };
        if let Some(sz) = size {
            if song.file_size == 0 {
                song.file_size = sz;
            }
        }
        out.push(song);

        if let Some(tx) = &progress {
            let _ = tx.send((idx + 1, total));
        }
    }

    let _ = ftp.quit();
    Ok(out)
}

/// 全库扫描。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    scan_dir(config, cover_cache, existing_modified, "", progress, read_tags)
}

// ==================== 读取 ====================

/// 取远端文件大小（`SIZE`；带路径失败时切到父目录再试，最后回到 `base`）。
///
/// 部分 FTP 服务器（尤其在 `SIZE` 实现上）不接受含斜杠的路径，因此需要这个回退。
pub fn size_of(ftp: &mut FtpStream, rel: &str, base: &str) -> Option<u64> {
    if let Ok(n) = ftp.size(rel) {
        return Some(n as u64);
    }
    let rel = normalize_rel(rel);
    let (dir, name) = rel.rsplit_once('/')?;
    ftp.cwd(dir).ok()?;
    let n = ftp.size(name).ok().map(|v| v as u64);
    // 回到起始目录，避免污染后续相对路径解析。
    let _ = ftp.cwd(base);
    n
}

/// 用已有会话读取文件头部若干字节并解析标签。
fn probe_with(
    ftp: &mut FtpStream,
    rel: &str,
    cover_cache: Option<&CoverCache>,
) -> Option<ScannedSong> {
    let mut collected: Vec<u8> = Vec::with_capacity(METADATA_PROBE_BYTES);
    let mut read_err: Option<String> = None;

    // 只读到探测所需长度就中断：`retr` 回调返回后服务器会关闭数据连接，
    // 因此这里的"提前结束"会带来一个可忽略的错误响应，只要拿到字节就继续。
    let result = ftp.retr(rel, |reader| {
        let mut chunk = vec![0u8; 64 * 1024];
        while collected.len() < METADATA_PROBE_BYTES {
            let want = (METADATA_PROBE_BYTES - collected.len()).min(chunk.len());
            match reader.read(&mut chunk[..want]) {
                Ok(0) => break,
                Ok(n) => collected.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    read_err = Some(e.to_string());
                    break;
                }
            }
        }
        Ok(())
    });

    if let Some(e) = read_err {
        log::debug!("[FTP] 探测读取中断 {rel}: {e}");
    }
    if collected.is_empty() {
        if let Err(e) = result {
            log::debug!("[FTP] 探测失败 {rel}: {e}");
        }
        return None;
    }

    let song = crate::utils::webdav::parse_probe_bytes(&collected, rel, None, cover_cache);
    (song.duration > 0.0).then_some(song)
}

/// 探测单首歌（后台补全用）。
pub fn probe_song(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    song_id: &str,
) -> Option<ScannedSong> {
    let mut ftp = connect(config).ok()?;
    let song = probe_with(&mut ftp, song_id, cover_cache);
    let _ = ftp.quit();
    song
}

/// 读取任意远端文件字节（封面等小文件）。
pub fn fetch_bytes(config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
    let mut ftp = connect(config)?;
    let result = ftp
        .retr_as_buffer(&normalize_rel(url))
        .map(|cursor| cursor.into_inner())
        .map_err(|e| format!("FTP 下载失败: {e}"));
    let _ = ftp.quit();
    result
}

/// 读取同名 `.lrc` 侧车歌词（无则 None）。
pub fn get_lyrics(config: &StreamServerConfig, song_id: &str) -> Option<String> {
    let lrc = swap_extension(song_id, "lrc");
    let bytes = fetch_bytes(config, &lrc).ok()?;
    String::from_utf8(bytes).ok()
}

/// 把路径的扩展名替换为 `ext`（只作用于最后一段文件名）。
pub fn swap_extension(path: &str, ext: &str) -> String {
    let (dir, name) = match path.rsplit_once('/') {
        Some((d, n)) => (Some(d), n),
        None => (None, path),
    };
    // 只认最后一段里的点：目录名里的点（如 `a.b/song`）不能当作扩展名。
    let new_name = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.{ext}"),
        _ => format!("{name}.{ext}"),
    };
    match dir {
        Some(d) => format!("{d}/{new_name}"),
        None => new_name,
    }
}

/// 把远端文件流式写入 sink，供 [`crate::source::bridge::SpoolReader`] 使用。
///
/// `offset > 0` 时通过 `REST` 命令从该偏移继续。
pub fn spool_segment(
    config: &StreamServerConfig,
    remote: &str,
    offset: u64,
    sink: &mut dyn Write,
    progress: &dyn Fn(u64),
    abort: &AtomicBool,
) -> Result<(), String> {
    let mut ftp = connect(config)?;

    if offset > 0 {
        ftp.resume_transfer(offset as usize)
            .map_err(|e| format!("FTP REST 失败（服务器可能不支持断点续传）: {e}"))?;
    }

    let mut written = 0u64;
    let mut io_err: Option<String> = None;
    let mut aborted = false;

    let result = ftp.retr(&normalize_rel(remote), |reader| {
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            if abort.load(Ordering::SeqCst) {
                aborted = true;
                break;
            }
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = sink.write_all(&buf[..n]) {
                        io_err = Some(format!("写入桥接缓存失败: {e}"));
                        break;
                    }
                    written += n as u64;
                    progress(written);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    io_err = Some(format!("FTP 读取失败: {e}"));
                    break;
                }
            }
        }
        Ok(())
    });

    // 主动中止 / 提前收尾会让服务器返回 4xx，属预期行为，不作为错误上报。
    let _ = ftp.quit();

    if let Some(e) = io_err {
        return Err(e);
    }
    if aborted {
        return Ok(());
    }
    result.map_err(|e| format!("FTP 传输失败: {e}"))?;
    Ok(())
}

/// 打开一个可随机读取的桥接读取器。
pub fn open_reader(
    config: &StreamServerConfig,
    song_id: &str,
) -> Result<Box<dyn crate::source::bridge::RangeReader>, String> {
    let total = {
        let mut ftp = connect(config)?;
        let size = size_of(&mut ftp, song_id, &base_dir(config));
        let _ = ftp.quit();
        size
    };

    let config = config.clone();
    let remote = song_id.to_string();
    let opener = std::sync::Arc::new(
        move |offset: u64,
              sink: &mut dyn Write,
              progress: &dyn Fn(u64),
              abort: &AtomicBool| {
            spool_segment(&config, &remote, offset, sink, progress, abort)
        },
    ) as std::sync::Arc<crate::source::bridge::SegmentOpener>;

    Ok(Box::new(crate::source::bridge::SpoolReader::new(total, opener)?))
}

/// 下载整首歌到本地缓存。
pub fn cache_song(
    config: &StreamServerConfig,
    cache_root: &Path,
    server_id: &str,
    song_id: &str,
) -> Result<String, String> {
    let dir = cache_root.join("webdav");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建缓存目录: {e}"))?;
    let path = crate::utils::webdav::cached_path(cache_root, server_id, song_id);
    if path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(path.to_string_lossy().to_string());
    }

    let bytes = fetch_bytes(config, song_id)?;
    let tmp_path = path.with_extension("part");
    if let Err(e) = std::fs::write(&tmp_path, &bytes) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("写入缓存失败: {e}"));
    }
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("落盘缓存失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 删除远端文件或目录。
pub fn delete(config: &StreamServerConfig, url: &str) -> Result<(), String> {
    let mut ftp = connect(config)?;
    let rel = normalize_rel(url);
    // 先按文件删；失败再按空目录删。
    let result = match ftp.rm(&rel) {
        Ok(()) => Ok(()),
        Err(_) => ftp
            .rmdir(&rel)
            .map_err(|e| format!("FTP 删除失败: {e}")),
    };
    let _ = ftp.quit();
    result
}

/// 移动 / 重命名远端条目。
pub fn move_entry(config: &StreamServerConfig, source: &str, destination: &str) -> Result<(), String> {
    let mut ftp = connect(config)?;
    let result = ftp
        .rename(normalize_rel(source), normalize_rel(destination))
        .map_err(|e| format!("FTP 移动失败: {e}"));
    let _ = ftp.quit();
    result
}

/// 上传本地文件到远端。
pub fn upload(config: &StreamServerConfig, local: &str, remote: &str) -> Result<(), String> {
    let mut file = std::fs::File::open(local).map_err(|e| format!("打开本地文件失败: {e}"))?;
    let mut ftp = connect(config)?;
    let result = ftp
        .put_file(normalize_rel(remote), &mut file)
        .map(|_| ())
        .map_err(|e| format!("FTP 上传失败: {e}"));
    let _ = ftp.quit();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_parsing_variants() {
        assert_eq!(parse_endpoint("ftp://nas.local:2121").unwrap(), ("nas.local".into(), 2121));
        assert_eq!(parse_endpoint("ftp://nas.local").unwrap(), ("nas.local".into(), 21));
        assert_eq!(parse_endpoint("nas.local:2121").unwrap(), ("nas.local".into(), 2121));
        assert_eq!(parse_endpoint("nas.local").unwrap(), ("nas.local".into(), 21));
        assert_eq!(parse_endpoint("192.168.1.10:21").unwrap(), ("192.168.1.10".into(), 21));
        // 带路径的裸地址也不应把路径当主机。
        assert_eq!(parse_endpoint("nas.local/music").unwrap(), ("nas.local".into(), 21));
        assert!(parse_endpoint("").is_err());
        assert!(parse_endpoint("nas.local:abc").is_err());
    }

    #[test]
    fn mlsd_line_parsing() {
        let e = parse_mlsd_line("type=file;size=41943040;modify=20240102030405; song.flac").unwrap();
        assert_eq!(e.name, "song.flac");
        assert!(!e.is_dir);
        assert_eq!(e.size, Some(41943040));
        assert_eq!(e.modified, parse_ftp_time("20240102030405"));

        let d = parse_mlsd_line("type=dir;modify=20230101000000; Album").unwrap();
        assert!(d.is_dir);
        assert_eq!(d.name, "Album");

        // 目录自引用项必须跳过，否则会无限递归。
        assert!(parse_mlsd_line("type=cdir; . ").is_none());
        assert!(parse_mlsd_line("type=pdir; ..").is_none());
        assert!(parse_mlsd_line("").is_none());
        assert!(parse_mlsd_line("   ").is_none());
    }

    #[test]
    fn mlsd_name_with_spaces_is_preserved() {
        let e = parse_mlsd_line("type=file;size=1; 01 - 歌 名.flac").unwrap();
        assert_eq!(e.name, "01 - 歌 名.flac");
    }

    #[test]
    fn ftp_time_parsing() {
        let t = parse_ftp_time("20240102030405").unwrap();
        assert_eq!(t, 1704164645, "应为 UTC 秒");
        // 带毫秒后缀也应接受（只取秒）。
        assert_eq!(parse_ftp_time("20240102030405.123"), Some(t));
        assert_eq!(parse_ftp_time("2024"), None);
        assert_eq!(parse_ftp_time("abcdefghijklmn"), None);
        assert_eq!(parse_ftp_time(""), None);
    }

    #[test]
    fn unix_list_line_parsing() {
        let e = parse_unix_list_line("-rw-r--r--   1 ftp   ftp    41943040 Jan 10 12:34 song.flac").unwrap();
        assert_eq!(e.name, "song.flac");
        assert!(!e.is_dir);
        assert_eq!(e.size, Some(41943040));

        let d = parse_unix_list_line("drwxr-xr-x   2 ftp   ftp        4096 Jan 10 12:34 Album").unwrap();
        assert!(d.is_dir);
        assert_eq!(d.name, "Album");

        // 文件名含空格：必须完整保留。
        let s = parse_unix_list_line("-rw-r--r-- 1 u g 123 Jan 10 2024 01 - My Song.flac").unwrap();
        assert_eq!(s.name, "01 - My Song.flac");

        // 非列表输出（如服务器横幅）应被拒绝。
        assert!(parse_unix_list_line("total 42").is_none());
        assert!(parse_unix_list_line("").is_none());
    }

    #[test]
    fn rel_path_helpers() {
        assert_eq!(normalize_rel("  /a/b/  "), "a/b");
        assert_eq!(normalize_rel("a\\b"), "a/b");
        assert_eq!(join_rel("", "x.flac"), "x.flac");
        assert_eq!(join_rel("a/b", "x.flac"), "a/b/x.flac");
        assert_eq!(join_rel("/a/", "/x.flac"), "a/x.flac");
    }

    #[test]
    fn swap_extension_for_lyrics() {
        assert_eq!(swap_extension("a/b/song.flac", "lrc"), "a/b/song.lrc");
        assert_eq!(swap_extension("song", "lrc"), "song.lrc");
    }
}
