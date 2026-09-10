//! 本地 / 已挂载路径音乐源（阶段 B2/B3）。
//!
//! ## 覆盖范围
//!
//! 本模块把"一个本机可访问的目录树"当作音乐源，因此天然覆盖三类场景：
//!
//! | 场景 | `server_url` 形态 | 说明 |
//! |---|---|---|
//! | **SMB / CIFS** | `\\nas\music`（Windows UNC）或 `/mnt/nas`（已挂载） | Windows 原生支持 UNC，凭据由系统处理；macOS/Linux 走 cifs 挂载 |
//! | **NFS** | `/mnt/nfs-music` | 走系统 nfs 挂载点 |
//! | 本地目录 / 移动硬盘 | `D:\Music` | 与远程共用同一套扫描 / 浏览 / 播放路径 |
//!
//! ## 为什么不内置 SMB2 协议实现
//!
//! 纯 Rust 的 SMB 客户端（`smb` / `smb2`）成熟度与维护状况都不足以作为桌面端
//! 主链路依赖，而绑定 `libsmbclient` 又会把构建绑死在各平台的本地库上（Windows
//! 需 MSYS2、移动端不可用）。走系统挂载点/UNC 则**零依赖、零凭据处理**，且与
//! 系统文件缓存、权限模型天然一致——这正是多数桌面播放器的做法。
//!
//! ## 播放路径
//!
//! 本地文件**不需要**走 [`crate::source::bridge`]：`AudioDecoder` 本来就支持
//! 直接 `File::open` 本地路径，且 `File` 天然可 seek。因此本连接器的
//! `get_stream_url` 直接返回绝对路径，零拷贝、零临时文件。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{ConnectionTestResult, ScannedSong, StreamServerConfig};
use crate::utils::cover::CoverCache;

/// 可被扫描入音乐的扩展名。
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "ape", "wv", "aiff", "aif", "alac", "dsf",
    "dff", "mka", "mp4", "wma",
];

/// 是否音频文件。
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// 解析配置对应的根目录（`server_url` 为共享根，`base_path` 为其下的起始子目录）。
pub fn root_path(config: &StreamServerConfig) -> Result<PathBuf, String> {
    let raw = config.server_url.trim();
    if raw.is_empty() {
        return Err("未填写共享路径（如 \\\\nas\\music 或 /mnt/nas）".to_string());
    }
    let mut path = PathBuf::from(raw);
    if let Some(sub) = config.base_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // 允许用户写 `/sub/dir` 或 `sub\dir`；`Path::join` 会正确处理分隔符。
        for part in sub.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".") {
            path.push(part);
        }
    }
    Ok(path)
}

/// 路径 → 稳定字符串（用于 song_id / 缓存键）。
fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// 文件修改时间（unix 秒）。
fn modified_secs(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// 测试连接：根目录必须存在且是目录。
pub fn test_connection(config: &StreamServerConfig) -> ConnectionTestResult {
    match root_path(config) {
        Ok(root) => {
            let meta = match std::fs::metadata(&root) {
                Ok(m) => m,
                Err(e) => {
                    return ConnectionTestResult {
                        success: false,
                        message: format!("无法访问 {}: {e}", root.display()),
                        server_version: None,
                    }
                }
            };
            if !meta.is_dir() {
                return ConnectionTestResult {
                    success: false,
                    message: format!("{} 不是目录", root.display()),
                    server_version: None,
                };
            }
            ConnectionTestResult {
                success: true,
                message: format!("已连接共享 {}", root.display()),
                server_version: None,
            }
        }
        Err(e) => ConnectionTestResult {
            success: false,
            message: e,
            server_version: None,
        },
    }
}

/// 列出目录内容（`dir_url` 为空取配置根目录）。
pub fn list_dir(
    config: &StreamServerConfig,
    dir_url: Option<&str>,
) -> Result<Vec<crate::source::DirEntry>, String> {
    let dir = match dir_url.map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => root_path(config)?,
    };

    let read = std::fs::read_dir(&dir).map_err(|e| format!("无法读取目录 {}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        // 跳过隐藏文件（.DS_Store、Thumbs.db 等）。
        if name.starts_with('.') {
            continue;
        }
        let is_dir = meta.is_dir();
        if !is_dir && !is_audio_file(&path) && !crate::utils::webdav::is_cover_image_name(&name) {
            continue;
        }
        out.push(crate::source::DirEntry {
            name,
            url: path_string(&path),
            is_dir,
            size: if is_dir { None } else { Some(meta.len()) },
            modified: modified_secs(&meta),
        });
    }

    // 目录在前，其次按名称排序，保证前端展示稳定。
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

/// 读取本地文件字节（封面等）。
pub fn fetch_bytes(_config: &StreamServerConfig, url: &str) -> Result<Vec<u8>, String> {
    std::fs::read(url).map_err(|e| format!("读取失败 {url}: {e}"))
}

/// 判断是否已是"根路径"形式，无需再拼配置根目录。
///
/// 注意：在 Windows 上 `Path::is_absolute()` 对 `/mnt/nas/x` 返回 false（缺少盘符），
/// 而 WebDAV/S3 风格的绝对路径、UNC 路径（`\\\\nas\\share`）都应以原样使用。
fn looks_absolute(raw: &str) -> bool {
    raw.starts_with('/') || raw.starts_with('\\') || Path::new(raw).is_absolute()
}

/// 把 `song_id` 解析成可播放的本地绝对路径。
///
/// 扫描入库时 `id` 就是绝对路径，因此绝大多数情况直接返回；仅在拿到相对路径
/// （例如手工构造）时才拼到配置根目录下。
pub fn resolve_path(config: &StreamServerConfig, song_id: &str) -> Result<String, String> {
    let raw = song_id.trim();
    if raw.is_empty() {
        return Err("空的歌曲路径".to_string());
    }
    if looks_absolute(raw) {
        return Ok(path_string(Path::new(raw)));
    }
    let mut joined = root_path(config)?;
    for part in raw.split(['/', '\\']).filter(|s| !s.is_empty()) {
        joined.push(part);
    }
    Ok(path_string(&joined))
}

/// 上传（复制）本地文件到共享目录。
pub fn upload(_config: &StreamServerConfig, local: &str, remote: &str) -> Result<(), String> {
    let dest = Path::new(remote);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    std::fs::copy(local, dest).map(|_| ()).map_err(|e| format!("复制到共享失败: {e}"))
}

/// 删除共享上的文件或空目录。
pub fn delete(_config: &StreamServerConfig, url: &str) -> Result<(), String> {
    let path = Path::new(url);
    let meta = std::fs::metadata(path).map_err(|e| format!("无法访问 {url}: {e}"))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| format!("删除目录失败: {e}"))
    } else {
        std::fs::remove_file(path).map_err(|e| format!("删除文件失败: {e}"))
    }
}

/// 移动 / 重命名共享上的条目。
pub fn move_entry(_config: &StreamServerConfig, source: &str, destination: &str) -> Result<(), String> {
    let dest = Path::new(destination);
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::rename(source, dest).map_err(|e| format!("移动失败: {e}"))
}

/// 探测单个本地文件（本地 IO 廉价，直接完整解析标签）。
pub fn probe_song(path: &str, cover_cache: Option<&CoverCache>) -> Option<ScannedSong> {
    let p = Path::new(path);
    if !p.is_file() {
        return None;
    }
    let modified = std::fs::metadata(p).ok().and_then(|m| modified_secs(&m));
    Some(crate::utils::webdav::parse_probe_file(
        p,
        path,
        modified,
        cover_cache,
    ))
}

/// 扫描一个目录（递归）下的全部音频文件。
///
/// `read_tags = false` 走"文件名入库"的快速模式，标签留给后台补全。
pub fn scan_dir(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    dir_url: &str,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let root = if dir_url.trim().is_empty() {
        root_path(config)?
    } else {
        PathBuf::from(dir_url.trim())
    };
    if !root.is_dir() {
        return Err(format!("目录不存在: {}", root.display()));
    }

    // 先收集待处理文件，才能给出准确的进度总数。
    let files: Vec<PathBuf> = walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| is_audio_file(p))
        .collect();

    let total = files.len();
    let mut out = Vec::with_capacity(total);

    for (idx, path) in files.iter().enumerate() {
        let key = path_string(path);
        let meta = std::fs::metadata(path).ok();
        let modified = meta.as_ref().and_then(modified_secs);

        // 增量：文件未变化则沿用库内既有元数据，跳过解析。
        let unchanged = match (existing_modified, modified) {
            (Some(map), Some(m)) => map.get(&key).map(|old| *old == m).unwrap_or(false),
            _ => false,
        };

        let song = if unchanged {
            crate::utils::webdav::fallback_song_with_modified(&key, modified)
        } else if read_tags {
            crate::utils::webdav::parse_probe_file(path, &key, modified, cover_cache)
        } else {
            crate::utils::webdav::fallback_song_with_modified(&key, modified)
        };

        // 本地文件的大小是现成的元数据，不依赖标签解析，直接补齐。
        let mut song = song;
        if let Some(meta) = &meta {
            if song.file_size == 0 {
                song.file_size = meta.len();
            }
        }
        out.push(song);

        if let Some(tx) = &progress {
            let _ = tx.send((idx + 1, total));
        }
    }

    Ok(out)
}

/// 全库扫描（从配置根目录递归）。
pub fn fetch_all_songs(
    config: &StreamServerConfig,
    cover_cache: Option<&CoverCache>,
    existing_modified: Option<&HashMap<String, i64>>,
    progress: Option<crate::source::ScanProgress>,
    read_tags: bool,
) -> Result<Vec<ScannedSong>, String> {
    let root = root_path(config)?;
    scan_dir(
        config,
        cover_cache,
        existing_modified,
        &path_string(&root),
        progress,
        read_tags,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ServerType;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 建一个隔离的临时目录，避免测试之间互相干扰。
    fn tmp_root() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("bayin-localfs-{}-{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn config_for(root: &Path, base: Option<&str>) -> StreamServerConfig {
        StreamServerConfig {
            server_type: ServerType::Smb,
            server_name: "nas".to_string(),
            server_url: root.to_string_lossy().to_string(),
            username: String::new(),
            password: String::new(),
            legacy_auth: false,
            music_folder_id: None,
            base_path: base.map(|s| s.to_string()),
            access_token: None,
            user_id: None,
        }
    }

    fn touch(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn root_path_joins_base_path_with_either_separator() {
        let root = PathBuf::from("/mnt/nas");
        let cfg = config_for(&root, Some("/music/flac"));
        assert_eq!(root_path(&cfg).unwrap(), PathBuf::from("/mnt/nas/music/flac"));

        let cfg2 = config_for(&root, Some("music\\flac"));
        assert_eq!(root_path(&cfg2).unwrap(), PathBuf::from("/mnt/nas/music/flac"));

        // 空 / 仅分隔符的 base_path 不应改变根。
        let cfg3 = config_for(&root, Some("  ///  "));
        assert_eq!(root_path(&cfg3).unwrap(), root);

        let cfg4 = config_for(&root, None);
        assert_eq!(root_path(&cfg4).unwrap(), root);
    }

    #[test]
    fn empty_server_url_is_rejected() {
        let cfg = config_for(Path::new(""), None);
        assert!(root_path(&cfg).is_err());
        assert!(!test_connection(&cfg).success);
    }

    #[test]
    fn test_connection_checks_directory_existence() {
        let root = tmp_root();
        assert!(test_connection(&config_for(&root, None)).success);

        let missing = root.join("does-not-exist");
        let res = test_connection(&config_for(&missing, None));
        assert!(!res.success);
        assert!(res.message.contains("无法访问"));

        // 指向文件而非目录时必须失败。
        let file = root.join("a.mp3");
        touch(&file, 10);
        assert!(!test_connection(&config_for(&file, None)).success);
    }

    #[test]
    fn list_dir_shows_dirs_and_audio_only() {
        let root = tmp_root();
        touch(&root.join("song.mp3"), 10);
        touch(&root.join("cover.jpg"), 10);
        touch(&root.join("notes.txt"), 10);
        std::fs::create_dir_all(root.join("album")).unwrap();

        let entries = list_dir(&config_for(&root, None), None).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"song.mp3"), "音频应出现在列表: {names:?}");
        assert!(names.contains(&"cover.jpg"), "封面图片应出现在列表（供浏览页展示）");
        assert!(names.contains(&"album"), "目录应出现在列表");
        assert!(!names.contains(&"notes.txt"), "非音频非图片文件应被过滤");

        // 目录排在最前，便于前端展示。
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "album");
    }

    #[test]
    fn scan_dir_collects_audio_recursively_and_marks_size() {
        let root = tmp_root();
        touch(&root.join("a.mp3"), 111);
        touch(&root.join("sub/b.flac"), 222);
        touch(&root.join("sub/readme.txt"), 10);

        let cfg = config_for(&root, None);
        let songs = scan_dir(&cfg, None, None, "", None, false).unwrap();
        let mut ids: Vec<String> = songs.iter().map(|s| s.id.clone()).collect();
        ids.sort();

        assert_eq!(ids.len(), 2, "应只收音频文件，实际 {ids:?}");
        assert!(ids[0].ends_with("a.mp3"));
        assert!(ids[1].ends_with("b.flac"));

        let sizes: Vec<u64> = songs.iter().map(|s| s.file_size).collect();
        assert!(sizes.contains(&111) && sizes.contains(&222), "应记录文件大小: {sizes:?}");
    }

    #[test]
    fn scan_dir_honours_subdirectory_as_scan_root() {
        let root = tmp_root();
        touch(&root.join("top.mp3"), 10);
        touch(&root.join("album/deep.mp3"), 10);

        let cfg = config_for(&root, None);
        let sub = root.join("album").to_string_lossy().to_string();
        let songs = scan_dir(&cfg, None, None, &sub, None, false).unwrap();
        assert_eq!(songs.len(), 1);
        assert!(songs[0].id.ends_with("deep.mp3"));
    }

    /// 增量扫描的关键约束：mtime 未变时不应重新解析标签（快速路径）。
    #[test]
    fn scan_dir_skips_unchanged_files_on_incremental() {
        let root = tmp_root();
        let file = root.join("a.mp3");
        touch(&file, 10);

        let cfg = config_for(&root, None);
        let mut existing = HashMap::new();
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        let secs = modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        existing.insert(file.to_string_lossy().to_string(), secs);

        let songs = scan_dir(&cfg, None, Some(&existing), "", None, true).unwrap();
        assert_eq!(songs.len(), 1);
        // 走了 fallback（未解析标签）→ 时长保持 0。
        assert_eq!(songs[0].duration, 0.0);
        assert_eq!(songs[0].file_modified, Some(secs));
    }

    #[test]
    fn missing_directory_is_an_error_not_empty_list() {
        let cfg = config_for(&tmp_root().join("nope"), None);
        assert!(scan_dir(&cfg, None, None, "", None, false).is_err());
        assert!(list_dir(&cfg, None).is_err());
    }

    #[test]
    fn fetch_bytes_reads_file_and_reports_missing() {
        let root = tmp_root();
        let file = root.join("cover.jpg");
        touch(&file, 7);
        let cfg = config_for(&root, None);
        assert_eq!(fetch_bytes(&cfg, &file.to_string_lossy()).unwrap().len(), 7);
        assert!(fetch_bytes(&cfg, "/definitely/missing").is_err());
    }

    #[test]
    fn audio_extension_detection() {
        assert!(is_audio_file(Path::new("/a/b.FLAC")));
        assert!(is_audio_file(Path::new("/a/b.mp3")));
        assert!(!is_audio_file(Path::new("/a/b.jpg")));
        assert!(!is_audio_file(Path::new("/a/b")));
    }
}
