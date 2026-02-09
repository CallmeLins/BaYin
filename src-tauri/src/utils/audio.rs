use std::path::Path;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use lofty::file::AudioFile;
use lofty::prelude::*;
use lofty::probe::Probe;

use crate::models::{ScannedSong, ScannedSongWithMtime};

/// 支持的音频文件扩展名
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aac", "m4a", "ogg", "wma", "ape", "aiff", "dsf", "dff",
];

/// 无损音频格式扩展名
const LOSSLESS_EXTENSIONS: &[&str] = &["flac", "wav", "ape", "aiff", "dsf", "dff"];

/// 判断文件是否为音频文件
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// 判断是否为无损格式
fn is_lossless_format(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| LOSSLESS_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// 从文件路径提取文件名（不含扩展名）
fn extract_filename(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "未知标题".to_string())
}

/// 从路径字符串中提取文件名（不含扩展名），用于流媒体等场景
pub fn extract_filename_from_path_str(path_str: &str) -> Option<String> {
    if path_str.is_empty() {
        return None;
    }

    // 处理 Windows 和 Unix 路径
    let path = std::path::Path::new(path_str);
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// 读取歌词（优先从外部 .lrc 文件，其次从音频文件内嵌歌词）
pub fn read_lyrics(audio_path: &Path) -> Option<String> {
    // 1. 尝试读取外部 .lrc 文件
    let lrc_path = audio_path.with_extension("lrc");
    if lrc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&lrc_path) {
            return Some(content);
        }
    }

    // 2. 尝试从音频文件读取内嵌歌词
    if let Ok(tagged_file) = Probe::open(audio_path).and_then(|p| p.read()) {
        if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
            // 尝试获取 LYRICS 标签（不同格式可能有不同的标签名）
            // lofty 使用 ItemKey::Lyrics 来获取歌词
            if let Some(lyrics) = tag.get_string(&lofty::tag::ItemKey::Lyrics) {
                return Some(lyrics.to_string());
            }
        }
    }

    None
}

/// 读取音频文件元数据
pub fn read_metadata(path: &Path) -> Result<ScannedSong, String> {
    let file_path_str = path.to_string_lossy().to_string();

    // 获取文件大小
    let file_size = std::fs::metadata(path)
        .map_err(|e| format!("无法获取文件信息: {}", e))?
        .len();

    // 使用 lofty 读取音频文件
    let tagged_file = Probe::open(path)
        .map_err(|e| format!("无法打开文件: {}", e))?
        .read()
        .map_err(|e| format!("无法读取音频文件: {}", e))?;

    // 获取音频属性
    let properties = tagged_file.properties();
    let duration = properties.duration().as_secs_f64();
    let sample_rate = properties.sample_rate().unwrap_or(0);
    let bit_depth = properties.bit_depth();

    // 判断音质
    let is_sq = is_lossless_format(path);
    let is_hr = sample_rate > 44100 || bit_depth.map(|d| d > 16).unwrap_or(false);

    // 获取标签信息
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| extract_filename(path));

    let artist = tag
        .and_then(|t| t.artist().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知艺术家".to_string());

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知专辑".to_string());

    // 提取封面
    let cover_url = tag.and_then(|t| {
        t.pictures().first().map(|pic| {
            let mime = pic.mime_type().map(|m| m.as_str()).unwrap_or("image/jpeg");
            let b64 = BASE64.encode(pic.data());
            format!("data:{};base64,{}", mime, b64)
        })
    });

    // 使用文件路径的哈希作为唯一 ID（确保同一文件每次扫描 ID 相同）
    let id = format!("{:x}", md5::compute(&file_path_str));

    Ok(ScannedSong {
        id,
        title,
        artist,
        album,
        duration,
        file_path: file_path_str,
        file_size,
        cover_url,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
    })
}

/// Read audio file metadata with modification time (for incremental scanning)
pub fn read_metadata_with_mtime(path: &Path) -> Result<ScannedSongWithMtime, String> {
    let file_path_str = path.to_string_lossy().to_string();

    // Get file metadata
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("无法获取文件信息: {}", e))?;

    let file_size = metadata.len();

    // Get file modification time as unix timestamp
    let file_modified = metadata
        .modified()
        .map_err(|e| format!("无法获取文件修改时间: {}", e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Use lofty to read audio file
    let tagged_file = Probe::open(path)
        .map_err(|e| format!("无法打开文件: {}", e))?
        .read()
        .map_err(|e| format!("无法读取音频文件: {}", e))?;

    // Get audio properties
    let properties = tagged_file.properties();
    let duration = properties.duration().as_secs_f64();
    let sample_rate = properties.sample_rate().unwrap_or(0);
    let bit_depth = properties.bit_depth();

    // Determine audio quality
    let is_sq = is_lossless_format(path);
    let is_hr = sample_rate > 44100 || bit_depth.map(|d| d > 16).unwrap_or(false);

    // Get tag information
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| extract_filename(path));

    let artist = tag
        .and_then(|t| t.artist().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知艺术家".to_string());

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知专辑".to_string());

    // Use file path hash as unique ID
    let id = format!("{:x}", md5::compute(&file_path_str));

    Ok(ScannedSongWithMtime {
        id,
        title,
        artist,
        album,
        duration,
        file_path: file_path_str,
        file_size,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
        file_modified,
    })
}

/// Get file modification time without reading full metadata
#[allow(dead_code)]
pub fn get_file_mtime(path: &Path) -> Result<i64, String> {
    std::fs::metadata(path)
        .map_err(|e| format!("无法获取文件信息: {}", e))?
        .modified()
        .map_err(|e| format!("无法获取文件修改时间: {}", e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| format!("时间转换错误: {}", e))
}
