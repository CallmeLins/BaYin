//! Cover image caching utilities
//!
//! Provides three-tier cover caching:
//! - small: 120x120 thumbnails for list views
//! - mid: 300x300 covers for album grids
//! - orig: Original resolution covers for full-screen view

use image::DynamicImage;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Cover size variants
#[derive(Debug, Clone, Copy)]
pub enum CoverSize {
    /// 120x120 thumbnail for list views
    Small,
    /// 300x300 for album/artist grids
    Mid,
    /// Original resolution
    Original,
}

/// Cover cache manager
#[derive(Clone)]
pub struct CoverCache {
    cache_dir: PathBuf,
}

impl CoverCache {
    /// Create a new cover cache manager
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get an Arc-wrapped clone for use in parallel processing
    pub fn clone_arc(&self) -> Arc<Self> {
        Arc::new(self.clone())
    }

    /// Get the cache directory for a given size
    fn size_dir(&self, size: CoverSize) -> PathBuf {
        match size {
            CoverSize::Small => self.cache_dir.join("small"),
            CoverSize::Mid => self.cache_dir.join("mid"),
            CoverSize::Original => self.cache_dir.join("orig"),
        }
    }

    /// Get the path for a cached cover by hash
    fn cover_path(&self, hash: &str, size: CoverSize, ext: &str) -> PathBuf {
        let prefix = &hash[..2.min(hash.len())];
        self.size_dir(size).join(prefix).join(format!("{}.{}", hash, ext))
    }

    /// Ensure cache directories exist
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.size_dir(CoverSize::Small))?;
        fs::create_dir_all(self.size_dir(CoverSize::Mid))?;
        fs::create_dir_all(self.size_dir(CoverSize::Original))?;
        Ok(())
    }

    /// Calculate hash of cover data
    pub fn hash_cover(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Save cover to cache (small, mid, and original)
    /// Returns the cover hash
    pub fn save_cover(&self, data: &[u8], mime_type: Option<&str>) -> Result<String, String> {
        let hash = Self::hash_cover(data);

        // Check if already cached
        let mid_path = self.cover_path(&hash, CoverSize::Mid, "jpg");
        if mid_path.exists() {
            return Ok(hash);
        }

        // Determine extension from mime type for original
        let ext = match mime_type {
            Some("image/png") => "png",
            Some("image/gif") => "gif",
            Some("image/webp") => "webp",
            _ => "jpg",
        };

        // Decode image
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        // Save original
        let orig_path = self.cover_path(&hash, CoverSize::Original, ext);
        if let Some(parent) = orig_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&orig_path, data).map_err(|e| e.to_string())?;

        // Create and save mid (300x300) - use faster filter
        let mid_img = img.resize_to_fill(300, 300, image::imageops::FilterType::Triangle);
        if let Some(parent) = mid_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        save_as_jpeg(&mid_img, &mid_path, 85)?;

        // Create and save small (120x120) - use faster filter
        let small_path = self.cover_path(&hash, CoverSize::Small, "jpg");
        let small_img = img.resize_to_fill(120, 120, image::imageops::FilterType::Triangle);
        if let Some(parent) = small_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        save_as_jpeg(&small_img, &small_path, 80)?;

        Ok(hash)
    }

    /// Get cover file path by hash and size
    pub fn get_cover_path(&self, hash: &str, size: CoverSize) -> Option<PathBuf> {
        // Try common extensions
        for ext in &["jpg", "png", "webp", "gif"] {
            let path = self.cover_path(hash, size, ext);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Get cover URL (asset protocol) by hash and size
    /// Uses http://asset.localhost/ format for Tauri 2.0
    pub fn get_cover_url(&self, hash: &str, size: CoverSize) -> Option<String> {
        self.get_cover_path(hash, size).map(|path| {
            let path_str = path.to_string_lossy().replace('\\', "/");
            // URL encode the colon in Windows drive letter (C: -> C%3A)
            let encoded_path = if path_str.len() > 1 && path_str.chars().nth(1) == Some(':') {
                format!("{}%3A{}", &path_str[0..1], &path_str[2..])
            } else {
                path_str
            };
            format!("http://asset.localhost/{}", encoded_path)
        })
    }

    /// Check if a cover exists in cache
    pub fn has_cover(&self, hash: &str) -> bool {
        self.get_cover_path(hash, CoverSize::Mid).is_some()
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        for size in [CoverSize::Small, CoverSize::Mid, CoverSize::Original] {
            let dir = self.size_dir(size);
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                            for sub_entry in sub_entries.flatten() {
                                if let Ok(meta) = sub_entry.metadata() {
                                    stats.file_count += 1;
                                    stats.total_size += meta.len();
                                }
                            }
                        }
                    }
                }
            }
        }

        stats
    }

    /// Clean up orphaned covers (covers not referenced by any song)
    pub fn cleanup_orphaned(&self, valid_hashes: &[String]) -> Result<usize, String> {
        let valid_set: std::collections::HashSet<_> = valid_hashes.iter().collect();
        let mut removed = 0;

        for size in [CoverSize::Small, CoverSize::Mid, CoverSize::Original] {
            let dir = self.size_dir(size);
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                            for sub_entry in sub_entries.flatten() {
                                let path = sub_entry.path();
                                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                    if !valid_set.contains(&stem.to_string()) {
                                        if fs::remove_file(&path).is_ok() {
                                            removed += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Clear all cached covers
    pub fn clear_all(&self) -> Result<usize, String> {
        let mut removed = 0;

        for size in [CoverSize::Small, CoverSize::Mid, CoverSize::Original] {
            let dir = self.size_dir(size);
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            if let Ok(count) = fs::read_dir(entry.path()).map(|e| e.count()) {
                                removed += count;
                            }
                            let _ = fs::remove_dir_all(entry.path());
                        }
                    }
                }
            }
        }

        Ok(removed)
    }
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub file_count: usize,
    pub total_size: u64,
}

/// Save image as JPEG with quality setting
fn save_as_jpeg(img: &DynamicImage, path: &Path, quality: u8) -> Result<(), String> {
    let rgb = img.to_rgb8();
    let mut buffer = Cursor::new(Vec::new());

    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
    encoder
        .encode_image(&rgb)
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    fs::write(path, buffer.into_inner()).map_err(|e| format!("Failed to write file: {}", e))
}

/// Extract cover from audio file and cache it
pub fn extract_and_cache_cover(
    audio_path: &Path,
    cache: &CoverCache,
) -> Result<Option<String>, String> {
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let tagged_file = Probe::open(audio_path)
        .map_err(|e| format!("Failed to open file: {}", e))?
        .read()
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    if let Some(tag) = tag {
        if let Some(pic) = tag.pictures().first() {
            let mime = pic.mime_type().map(|m| m.as_str());
            let hash = cache.save_cover(pic.data(), mime)?;
            return Ok(Some(hash));
        }
    }

    Ok(None)
}

/// Download and cache cover from URL
pub async fn download_and_cache_cover(
    url: &str,
    cache: &CoverCache,
) -> Result<Option<String>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to download: {}", e))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let data = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if data.is_empty() {
        return Ok(None);
    }

    let hash = cache.save_cover(&data, content_type.as_deref())?;
    Ok(Some(hash))
}
