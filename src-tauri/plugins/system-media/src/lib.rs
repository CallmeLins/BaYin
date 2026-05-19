#![allow(unexpected_cfgs)]
#![allow(unused_imports)]

use std::sync::Mutex;
use tauri::{plugin::TauriPlugin, Emitter, Manager, Runtime};

pub mod models;
mod platform;

use models::*;

#[cfg(target_os = "android")]
const ANDROID_PLUGIN_ID: &str = "app.tauri.bayin.systemmedia";

/// Re-exported on Android only. The main app must call this once at startup
/// so the cdylib linker keeps the JNI export symbols in `libbayin_lib.so`
/// (without a reachable reference they get stripped from the dependency rlib).
#[cfg(target_os = "android")]
pub fn ensure_jni_exports_linked() {
    platform::android::ensure_jni_exports_linked();
}

// ── Plugin state ───────────────────────────────────────────────────

struct MediaState {
    controller: Mutex<Box<dyn platform::MediaController>>,
}

// ── Tauri commands ─────────────────────────────────────────────────

#[tauri::command]
fn initialize(state: tauri::State<'_, MediaState>) -> Result<(), String> {
    let mut ctrl = state.controller.lock().map_err(|e| e.to_string())?;
    ctrl.initialize().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_metadata(
    state: tauri::State<'_, MediaState>,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    artwork_url: Option<String>,
) -> Result<(), String> {
    let mut ctrl = state.controller.lock().map_err(|e| e.to_string())?;
    let meta = MediaMetadata { title, artist, album, duration, artwork_url };
    ctrl.set_metadata(&meta).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_playback_status(
    state: tauri::State<'_, MediaState>,
    status: PlaybackStatus,
) -> Result<(), String> {
    let mut ctrl = state.controller.lock().map_err(|e| e.to_string())?;
    ctrl.set_playback_status(status).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_position(
    state: tauri::State<'_, MediaState>,
    position_secs: f64,
) -> Result<(), String> {
    let mut ctrl = state.controller.lock().map_err(|e| e.to_string())?;
    ctrl.set_position(position_secs).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear(state: tauri::State<'_, MediaState>) -> Result<(), String> {
    let mut ctrl = state.controller.lock().map_err(|e| e.to_string())?;
    ctrl.clear().map_err(|e| e.to_string())
}

// ── Plugin init ────────────────────────────────────────────────────

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("bayin-system-media")
        .invoke_handler(tauri::generate_handler![
            initialize,
            set_metadata,
            set_playback_status,
            set_position,
            clear,
        ])
        .setup(|_app, _api| {
            // Desktop and Android both use the cross-platform MediaController
            // API. On desktop the controller talks to the OS media API
            // directly; on Android it writes to a shared state that the
            // Kotlin foreground service polls (see platform/android.rs).
            #[cfg(any(desktop, target_os = "android"))]
            {
                let controller = platform::create_controller();

                // Register event handler that emits Tauri events to the frontend.
                let app_handle = _app.clone();
                let event_sink: Box<dyn Fn(MediaControlEvent) + Send> =
                    Box::new(move |event: MediaControlEvent| {
                        let _ = app_handle.emit("bayin-system-media://control", &event);
                    });

                #[cfg(target_os = "macos")]
                platform::macos::set_event_handler(event_sink);

                #[cfg(target_os = "windows")]
                platform::windows::set_event_handler(event_sink);

                #[cfg(target_os = "linux")]
                platform::linux::set_event_handler(event_sink);

                #[cfg(target_os = "android")]
                platform::android::set_event_handler(event_sink);

                _app.manage(MediaState {
                    controller: Mutex::new(controller),
                });
            }

            // Android: register the Kotlin-side plugin (MediaSession + foreground
            // notification service). Must run AFTER the event handler is set so
            // any immediate JNI callback finds the sink populated.
            #[cfg(target_os = "android")]
            {
                let _ = _api.register_android_plugin(ANDROID_PLUGIN_ID, "SystemMediaPlugin");
            }

            Ok(())
        })
        .build()
}
