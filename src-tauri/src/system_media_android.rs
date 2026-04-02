//! Android system media integration glue.
//!
//! Kotlin `MediaSession` callbacks (from a ForegroundService) cannot depend on the WebView being
//! alive, so we expose a small JNI surface that can directly control the Rust playback core.

#![cfg(target_os = "android")]

use std::sync::OnceLock;
use std::{
    collections::HashSet,
    io::Read,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use jni::{
    objects::{JClass, JString},
    sys::{jlong, jstring},
    JNIEnv,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
use crate::commands::CoverCacheState;
use crate::db::DbState;
use crate::playback::PlaybackDomainState;
use crate::playback_control;
use crate::utils::cover::CoverSize;

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
static ARTWORK_CACHE_INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

#[derive(Debug, Clone, Serialize)]
struct AndroidNowPlaying {
    // A stable id for the system UI (file path is ok for local library).
    id: String,
    title: String,
    artist: String,
    album: String,
    artwork_path: Option<String>,
    artwork_url: Option<String>,
    is_playing: bool,
    position_ms: i64,
    duration_ms: i64,
    can_next: bool,
    can_previous: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DomainChangedPayload {
    index: usize,
    track_id: String,
}

fn emit_domain_changed(app: &tauri::AppHandle) {
    let domain = match app.try_state::<PlaybackDomainState>() {
        Some(s) => s,
        None => return,
    };
    let (index, track_id) = {
        let d = domain.0.lock().unwrap();
        if d.queue.is_empty() || d.index >= d.queue.len() {
            return;
        }
        (d.index, d.queue[d.index].id.clone())
    };
    let _ = app.emit(
        "playback:domain_changed",
        DomainChangedPayload { index, track_id },
    );
}

fn inflight_set() -> MutexGuard<'static, HashSet<String>> {
    ARTWORK_CACHE_INFLIGHT
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap()
}

fn try_mark_artwork_inflight(track_id: &str) -> bool {
    let mut set = inflight_set();
    if set.contains(track_id) {
        return false;
    }
    set.insert(track_id.to_string());
    true
}

fn unmark_artwork_inflight(track_id: &str) {
    let mut set = inflight_set();
    set.remove(track_id);
}

fn spawn_lazy_cache_artwork(app: tauri::AppHandle, track_id: String, artwork_url: String) {
    if !try_mark_artwork_inflight(&track_id) {
        return;
    }

    std::thread::spawn(move || {
        let result: Result<String, String> = (|| {
            // 1) Download
            let client = reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

            let resp = client
                .get(&artwork_url)
                .send()
                .map_err(|e| format!("Artwork request failed: {}", e))?;

            let status = resp.status().as_u16();
            if status != 200 {
                return Err(format!("Artwork request failed with status {}", status));
            }

            let mime_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            // Avoid pathological downloads; covers should be small.
            let max_bytes: usize = 8 * 1024 * 1024;
            let mut buf = Vec::new();
            resp.take(max_bytes as u64)
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read artwork bytes: {}", e))?;
            if buf.is_empty() {
                return Err("Empty artwork response".to_string());
            }

            // 2) Save into CoverCache
            let hash = {
                let cache = app
                    .try_state::<CoverCacheState>()
                    .ok_or_else(|| "CoverCacheState not available".to_string())?;
                let cache = cache.0.lock().map_err(|_| "CoverCache lock poisoned".to_string())?;
                cache.save_cover(&buf, mime_type.as_deref())
            }?;

            // 3) Persist into DB
            {
                let db = app
                    .try_state::<DbState>()
                    .ok_or_else(|| "DbState not available".to_string())?;
                let conn = db.0.lock().map_err(|_| "DbState lock poisoned".to_string())?;
                let _ = crate::db::songs::update_song_cover_hash(&conn, &track_id, &hash)
                    .map_err(|e| e.to_string())?;
            }

            // 4) Patch playback domain so subsequent polls can return `artwork_path` without waiting for UI.
            if let Some(domain) = app.try_state::<PlaybackDomainState>() {
                let mut d = domain.0.lock().unwrap();
                if let Some(t) = d.queue.iter_mut().find(|t| t.id == track_id) {
                    t.artwork_ref = Some(hash.clone());
                }
            }

            Ok(hash)
        })();

        unmark_artwork_inflight(&track_id);

        if let Ok(hash) = result {
            // Optional: UI may choose to refresh covers when this fires.
            let _ = app.emit(
                "playback:artwork_cached",
                serde_json::json!({ "trackId": track_id, "coverHash": hash }),
            );
        }
    });
}

fn now_playing_snapshot(app: &tauri::AppHandle) -> Option<AndroidNowPlaying> {
    let domain = app.try_state::<PlaybackDomainState>()?;
    let engine = app.try_state::<AudioEngineState>()?;

    let (track, queue_len) = {
        let d = domain.0.lock().unwrap();
        let t = d.queue.get(d.index)?.clone();
        (t, d.queue.len())
    };

    let artwork_path = track.artwork_ref.as_ref().and_then(|hash| {
        let cache = app.try_state::<CoverCacheState>()?;
        let cache = cache.0.lock().ok()?;
        cache
            .get_cover_path(hash, CoverSize::Mid)
            .map(|p| p.to_string_lossy().to_string())
    });

    // Lazy cover caching for streaming tracks:
    // - If we already have a cached `cover_hash`, prefer local `artwork_path`.
    // - Otherwise, return `artwork_url` immediately (so system UI can show something),
    //   while caching the image into CoverCache + DB in the background.
    if artwork_path.is_none() && track.artwork_ref.is_none() {
        if let Some(url) = track.artwork_url.clone() {
            spawn_lazy_cache_artwork(app.clone(), track.id.clone(), url);
        }
    }

    let s = {
        let engine = engine.lock().unwrap();
        let s = engine.state.lock().unwrap().clone();
        s
    };

    let pos_ms = (s.position_secs.max(0.0) * 1000.0).round() as i64;
    let dur_secs = if track.duration_secs > 0.0 {
        track.duration_secs
    } else {
        s.duration_secs
    };
    let dur_ms = (dur_secs.max(0.0) * 1000.0).round() as i64;

    Some(AndroidNowPlaying {
        id: track.id,
        title: track.title,
        artist: track.artist,
        album: track.album,
        artwork_path,
        artwork_url: track.artwork_url,
        is_playing: s.is_playing,
        position_ms: pos_ms,
        duration_ms: dur_ms,
        can_next: queue_len > 1,
        can_previous: queue_len > 1,
    })
}

fn with_app<F: FnOnce(&tauri::AppHandle)>(f: F) {
    if let Some(app) = APP_HANDLE.get() {
        f(app);
    }
}

fn with_app_ret<T, F: FnOnce(&tauri::AppHandle) -> T>(f: F) -> Option<T> {
    APP_HANDLE.get().map(f)
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeGetNowPlayingJson(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let json = with_app_ret(|app| now_playing_snapshot(app))
        .flatten()
        .and_then(|s| serde_json::to_string(&s).ok())
        .unwrap_or_else(|| "{}".to_string());

    match env.new_string(json) {
        Ok(s) => s.into_raw(),
        Err(_) => JString::default().into_raw(),
    }
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePlay(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let domain = match app.try_state::<PlaybackDomainState>() {
            Some(s) => s,
            None => return,
        };
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };

        // If nothing is loaded yet, kick off playing the current index.
        let st = {
            let engine = engine.lock().unwrap();
            let s = engine.state.lock().unwrap().clone();
            s
        };
        if st.duration_secs <= 0.0 && st.position_secs <= 0.0 {
            let idx = { domain.0.lock().unwrap().index };
            let _ = playback_control::play_index(idx, &domain, &engine);
            emit_domain_changed(app);
        } else {
            let engine = engine.lock().unwrap();
            engine.send(AudioCommand::Resume);
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePause(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let engine = engine.lock().unwrap();
        engine.send(AudioCommand::Pause);
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeResume(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let engine = engine.lock().unwrap();
        engine.send(AudioCommand::Resume);
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeTogglePlayPause(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let st = {
            let engine = engine.lock().unwrap();
            let s = engine.state.lock().unwrap().clone();
            s
        };
        let engine = engine.lock().unwrap();
        if st.is_playing {
            engine.send(AudioCommand::Pause);
        } else {
            engine.send(AudioCommand::Resume);
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeNext(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let domain = match app.try_state::<PlaybackDomainState>() {
            Some(s) => s,
            None => return,
        };
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let _ = playback_control::next(&domain, &engine);
        emit_domain_changed(app);
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePrevious(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let domain = match app.try_state::<PlaybackDomainState>() {
            Some(s) => s,
            None => return,
        };
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let _ = playback_control::previous(&domain, &engine);
        emit_domain_changed(app);
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeSeekToMs(
    _env: JNIEnv,
    _class: JClass,
    position_ms: jlong,
) {
    let position_secs = (position_ms.max(0) as f64) / 1000.0;
    with_app(|app| {
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let engine = engine.lock().unwrap();
        engine.send(AudioCommand::Seek { position_secs });
    });
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeRefreshOutput(
    _env: JNIEnv,
    _class: JClass,
) {
    with_app(|app| {
        let engine = match app.try_state::<AudioEngineState>() {
            Some(s) => s,
            None => return,
        };
        let engine = engine.lock().unwrap();
        engine.send(AudioCommand::RefreshOutput);
    });
}
