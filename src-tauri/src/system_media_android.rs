//! Android system media integration glue.
//!
//! Kotlin `MediaSession` callbacks (from a ForegroundService) cannot depend on the WebView being
//! alive, so we expose a small JNI surface that can directly control the Rust playback core.

#![cfg(target_os = "android")]

use std::sync::OnceLock;

use jni::{
    objects::{JClass, JString},
    sys::{jlong, jstring},
    JNIEnv,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
use crate::commands::CoverCacheState;
use crate::playback::PlaybackDomainState;
use crate::playback_control;
use crate::utils::cover::CoverSize;

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

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
