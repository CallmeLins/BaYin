use std::path::Path;
use std::io::Read;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use flate2::read::ZlibDecoder;
use lofty::file::AudioFile;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, ItemValue, Tag};

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

    // 1b. 尝试读取外部 .krc (KuGou) 逐字歌词，并转换成 LRC + 绝对时间 token 格式
    let krc_path = audio_path.with_extension("krc");
    if krc_path.exists() {
        if let Some(content) = read_krc_as_lrc(&krc_path) {
            return Some(content);
        }
    }

    // 2. 尝试从音频文件读取内嵌歌词
    if let Ok(tagged_file) = Probe::open(audio_path).and_then(|p| p.read()) {
        // Prefer scanning all tags instead of just the primary tag, because FLAC can carry
        // multiple tag blocks (Vorbis comments, ID3v2, etc.), and lyrics may live in a non-primary tag.
        let mut best: Option<String> = None;

        for tag in tagged_file.tags() {
            if let Some(lyrics) = extract_lyrics_from_tag(tag) {
                // If we found something that looks like time-tagged lyrics, return immediately.
                if looks_time_tagged(&lyrics) || looks_like_krc_plaintext(&lyrics) {
                    return Some(normalize_lyrics_text(&lyrics));
                }
                best = best.or(Some(lyrics));
            }
        }

        if let Some(fallback) = best {
            return Some(normalize_lyrics_text(&fallback));
        }
    }

    None
}

fn looks_time_tagged(s: &str) -> bool {
    // Cheap checks for LRC and karaoke tags.
    s.contains('[') && s.contains(':') && s.contains(']')
}

fn looks_like_krc_plaintext(s: &str) -> bool {
    // KuGou KRC plaintext has headers like `[12345,678]` and tokens like `<0,500,0>`.
    if !s.contains('[') || !s.contains(',') || !s.contains(']') || !s.contains('<') || !s.contains('>') {
        return false;
    }
    // Guard against normal LRC `<mm:ss.xxx>` which also contains ',' rarely.
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            let mut saw_digit = false;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                saw_digit = true;
                j += 1;
            }
            if saw_digit && j < bytes.len() && bytes[j] == b',' {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn normalize_lyrics_text(s: &str) -> String {
    if looks_like_krc_plaintext(s) {
        // Embedded KRC-like plaintext (already decompressed) is common in some taggers.
        // Convert it so the frontend can parse and render word-by-word karaoke.
        return krc_text_to_lrc(s);
    }

    s.to_string()
}

fn extract_lyrics_from_tag(tag: &Tag) -> Option<String> {
    // 1) Standard mapped lyrics key.
    if let Some(lyrics) = tag.get_string(&ItemKey::Lyrics) {
        return Some(lyrics.to_string());
    }

    // 2) Try common "lyrics-like" unknown keys (e.g. Vorbis comments such as LYRICS, UNSYNCEDLYRICS).
    for item in tag.items() {
        let key = item.key();
        let key_name = match key {
            ItemKey::Unknown(s) => s.as_str(),
            _ => continue,
        };

        let upper = key_name.to_ascii_uppercase();
        if !upper.contains("LYRIC") && !upper.contains("LRC") {
            continue;
        }

        // Prefer text values; some taggers store lyrics as binary bytes.
        let v = item.value();
        if let ItemValue::Text(text) = v {
            if !text.is_empty() {
                return Some(text.to_string());
            }
        } else if let ItemValue::Binary(data) = v {
            if let Ok(text) = std::str::from_utf8(&data) {
                let t = text.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }

    None
}

/// Decode KuGou `.krc` lyrics to a LRC-like format that the frontend can parse and render
/// as word-by-word (karaoke) lyrics.
///
/// KRC format:
/// - File starts with `krc1` header
/// - Remaining bytes are XOR-obfuscated and then zlib-compressed
/// - Decompressed text contains lines like: `[start_ms,duration_ms]<offset_ms,dur_ms,0>词...`
///
/// We convert to:
/// - Line timestamp: `[mm:ss.mmm]`
/// - Word timestamps: `<mm:ss.mmm>` absolute times, so the existing LRC karaoke parser works.
fn read_krc_as_lrc(krc_path: &Path) -> Option<String> {
    let data = std::fs::read(krc_path).ok()?;
    if data.len() < 8 || &data[0..4] != b"krc1" {
        return None;
    }

    // KuGou KRC XOR key (repeats).
    const KEY: [u8; 16] = [
        0x40, 0x47, 0x61, 0x77, 0x5E, 0x32, 0x74, 0x47,
        0x51, 0x36, 0x31, 0x2D, 0xCE, 0xD2, 0x6E, 0x69,
    ];

    let mut buf = data[4..].to_vec();
    for (i, b) in buf.iter_mut().enumerate() {
        *b ^= KEY[i % KEY.len()];
    }

    let mut decoder = ZlibDecoder::new(&buf[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).ok()?;
    let text = String::from_utf8(decompressed).ok()?;

    Some(krc_text_to_lrc(&text))
}

fn ms_to_timestamp(ms: u64) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms / 1_000) % 60;
    let millis = ms % 1_000;
    format!("{:02}:{:02}.{:03}", minutes, seconds, millis)
}

fn krc_text_to_lrc(text: &str) -> String {
    let mut out = String::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Skip metadata like [ti:], [ar:], [id:], etc.
        if line.starts_with('[') && line.contains(':') {
            continue;
        }

        // Parse `[start_ms,duration_ms]...`
        let Some(close_bracket) = line.find(']') else { continue };
        if !line.starts_with('[') || close_bracket < 3 {
            continue;
        }

        let header = &line[1..close_bracket];
        let mut parts = header.split(',');
        let start_ms: u64 = match parts.next().and_then(|v| v.parse().ok()) {
            Some(v) => v,
            None => continue,
        };
        let _dur_ms: u64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);

        let body = &line[(close_bracket + 1)..];

        // Build absolute-time karaoke tags: `<mm:ss.mmm>词`
        let mut rendered = String::new();
        let mut idx = 0usize;
        let bytes = body.as_bytes();
        while idx < bytes.len() {
            if bytes[idx] != b'<' {
                idx += 1;
                continue;
            }

            let Some(tag_end_rel) = body[idx..].find('>') else { break };
            let tag = &body[(idx + 1)..(idx + tag_end_rel)];
            let mut tag_parts = tag.split(',');
            let offset_ms: u64 = match tag_parts.next().and_then(|v| v.parse().ok()) {
                Some(v) => v,
                None => {
                    idx += tag_end_rel + 1;
                    continue;
                }
            };

            let text_start = idx + tag_end_rel + 1;
            let next_tag = body[text_start..]
                .find('<')
                .map(|p| text_start + p)
                .unwrap_or(body.len());
            let seg = body[text_start..next_tag].replace('\r', "");

            let abs_ms = start_ms.saturating_add(offset_ms);
            rendered.push('<');
            rendered.push_str(&ms_to_timestamp(abs_ms));
            rendered.push('>');
            rendered.push_str(&seg);

            idx = next_tag;
        }

        let ts = ms_to_timestamp(start_ms);
        if !rendered.is_empty() {
            out.push_str(&format!("[{}]{}\n", ts, rendered.trim_end()));
        } else if !body.trim().is_empty() {
            out.push_str(&format!("[{}]{}\n", ts, body.trim()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::krc_text_to_lrc;

    #[test]
    fn converts_krc_line_to_lrc_karaoke() {
        let krc = "[1000,2000]<0,500,0>你<500,500,0>好";
        let lrc = krc_text_to_lrc(krc);
        assert!(lrc.contains("[00:01.000]"));
        assert!(lrc.contains("<00:01.000>你"));
        assert!(lrc.contains("<00:01.500>好"));
    }
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
    let bitrate = properties.audio_bitrate();
    let channels = properties.channels().map(|c| c as u8);

    // 获取文件格式（从扩展名）
    let format = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_uppercase());

    // 判断音质
    let is_sq = is_lossless_format(path);
    let is_hr = sample_rate > 44100 || bit_depth.map(|d| d > 16).unwrap_or(false);

    // 获取标签信息
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

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
        cover_hash: None,
        file_modified: None,
        is_hr: Some(is_hr),
        is_sq: Some(is_sq),
        format,
        bit_depth: bit_depth.map(|d| d as u8),
        sample_rate: if sample_rate > 0 {
            Some(sample_rate)
        } else {
            None
        },
        bitrate,
        channels,
        created_at: None,
    })
}

/// Read audio file metadata with modification time (for incremental scanning)
pub fn read_metadata_with_mtime(path: &Path) -> Result<ScannedSongWithMtime, String> {
    let file_path_str = path.to_string_lossy().to_string();

    // Get file metadata
    let metadata = std::fs::metadata(path).map_err(|e| format!("无法获取文件信息: {}", e))?;

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
    let bitrate = properties.audio_bitrate();
    let channels = properties.channels().map(|c| c as u8);

    // Get file format from extension
    let format = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_uppercase());

    // Determine audio quality
    let is_sq = is_lossless_format(path);
    let is_hr = sample_rate > 44100 || bit_depth.map(|d| d > 16).unwrap_or(false);

    // Get tag information
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

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
        format,
        bit_depth: bit_depth.map(|d| d as u8),
        sample_rate: if sample_rate > 0 {
            Some(sample_rate)
        } else {
            None
        },
        bitrate,
        channels,
        file_modified,
    })
}
