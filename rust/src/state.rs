use crate::audio_engine::AudioEngine;
use crate::utils::cover::CoverCache;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

static DATABASE: LazyLock<Mutex<Option<Connection>>> = LazyLock::new(|| Mutex::new(None));
static AUDIO_ENGINE: LazyLock<Mutex<Option<AudioEngine>>> = LazyLock::new(|| Mutex::new(None));
static COVER_CACHE: LazyLock<Mutex<Option<CoverCache>>> = LazyLock::new(|| Mutex::new(None));

const DB_NOT_INITIALIZED: &str = "Database not initialized. Call init_db() first.";
const AUDIO_ENGINE_NOT_INITIALIZED: &str =
    "Audio engine not initialized. Call ensure_audio_engine() first.";
const COVER_CACHE_NOT_INITIALIZED: &str =
    "Cover cache not initialized. Call init_cover_cache() first.";

pub fn init_database(path: &Path) -> Result<(), String> {
    let connection = crate::db::open_db(path).map_err(|e| e.to_string())?;
    let mut guard = DATABASE.lock().map_err(|e| e.to_string())?;
    *guard = Some(connection);
    Ok(())
}

pub fn with_db<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let guard = DATABASE.lock().map_err(|e| e.to_string())?;
    let connection = guard
        .as_ref()
        .ok_or_else(|| DB_NOT_INITIALIZED.to_string())?;
    f(connection)
}

pub fn with_db_mut<T>(f: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
    let mut guard = DATABASE.lock().map_err(|e| e.to_string())?;
    let connection = guard
        .as_mut()
        .ok_or_else(|| DB_NOT_INITIALIZED.to_string())?;
    f(connection)
}

pub fn ensure_audio_engine() -> Result<(), String> {
    let mut guard = AUDIO_ENGINE.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(AudioEngine::new()?);
    }
    Ok(())
}

pub fn with_audio_engine<T>(
    f: impl FnOnce(&AudioEngine) -> Result<T, String>,
) -> Result<T, String> {
    ensure_audio_engine()?;
    let guard = AUDIO_ENGINE.lock().map_err(|e| e.to_string())?;
    let engine = guard
        .as_ref()
        .ok_or_else(|| AUDIO_ENGINE_NOT_INITIALIZED.to_string())?;
    f(engine)
}

pub fn init_cover_cache(cache_dir: PathBuf) -> Result<(), String> {
    let cache = CoverCache::new(cache_dir);
    cache.ensure_dirs().map_err(|e| e.to_string())?;
    let mut guard = COVER_CACHE.lock().map_err(|e| e.to_string())?;
    *guard = Some(cache);
    Ok(())
}

pub fn with_cover_cache<T>(f: impl FnOnce(&CoverCache) -> Result<T, String>) -> Result<T, String> {
    let guard = COVER_CACHE.lock().map_err(|e| e.to_string())?;
    let cache = guard
        .as_ref()
        .ok_or_else(|| COVER_CACHE_NOT_INITIALIZED.to_string())?;
    f(cache)
}
