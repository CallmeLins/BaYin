//! Android implementation.
//!
//! Architecture overview (different from desktop):
//!
//! The MediaSession + foreground notification lives in Kotlin
//! (`MediaPlaybackService.kt`). It polls Rust once a second via the
//! `SystemMediaBridge` JNI methods and writes the polled state into the
//! MediaSession. User actions from the lock screen / notification trigger
//! MediaSession callbacks which also call into JNI to forward the action
//! back to Rust.
//!
//! On the Rust side:
//!
//! * `AndroidController` implements `MediaController` like the desktop
//!   platforms, but instead of talking to a system API directly it writes
//!   the latest "now playing" snapshot into a shared `Mutex<NowPlayingState>`
//!   that the JNI exports below read out as JSON.
//! * JNI control callbacks (`nativePlay`, `nativeNext`, ...) emit
//!   `MediaControlEvent`s through the same global event sink that the
//!   plugin's setup hook wired to a Tauri event, so the JS frontend
//!   receives `bayin-system-media://control` events identically to desktop.
//!
//! Symbol export: cdylib linkage strips `#[no_mangle]` items defined in
//! dependency rlibs unless they are referenced from a reachable code path.
//! `ensure_jni_exports_linked()` is a no-op anchor function that touches
//! every JNI export; the main app must call it once at startup so the
//! linker keeps these symbols in `libbayin_lib.so`.

use super::MediaController;
use crate::models::*;
use std::error::Error as StdError;
use std::sync::{Arc, Mutex, OnceLock};

// ── Shared "now playing" state ─────────────────────────────────────
#[derive(Default, Clone)]
struct NowPlayingState {
    id: String,
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
    position_ms: i64,
    /// Local file path (or empty if remote / not set).
    artwork_path: String,
    /// Remote URL (or empty if local / not set).
    artwork_url: String,
    is_playing: bool,
    can_next: bool,
    can_previous: bool,
}

impl NowPlayingState {
    fn fresh() -> Self {
        Self {
            can_next: true,
            can_previous: true,
            ..Default::default()
        }
    }
}

// ── Rust → Java push notification ──────────────────────────────────
//
// Service used to poll Rust every 1 s, which left a 1 s lag between
// seek/pause and the notification refresh. We now invoke a Java Runnable
// from Rust whenever the state changes, so the Service updates immediately.
// (Polling stays as a slow safety net in case the callback ref is lost.)
use jni::objects::GlobalRef;
use jni::JavaVM;

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
static UPDATE_CALLBACK: OnceLock<Mutex<Option<GlobalRef>>> = OnceLock::new();

fn callback_slot() -> &'static Mutex<Option<GlobalRef>> {
    UPDATE_CALLBACK.get_or_init(|| Mutex::new(None))
}

/// Invoke the Service's registered Runnable on the current thread. Cheap:
/// the Kotlin side immediately re-dispatches to the Service executor.
fn notify_update() {
    let vm = match JAVA_VM.get() {
        Some(v) => v,
        None => return,
    };
    let cb = {
        let guard = match callback_slot().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        match guard.as_ref() {
            Some(c) => c.clone(),
            None => return,
        }
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[system-media] vm.attach_current_thread failed: {:?}", e);
            return;
        }
    };
    if let Err(e) = env.call_method(cb.as_obj(), "run", "()V", &[]) {
        log::warn!("[system-media] update callback invoke failed: {:?}", e);
        // Clear any pending Java exception so subsequent JNI calls don't fail.
        let _ = env.exception_clear();
    }
}

// ── Shared state accessor for the JNI poll path ────────────────────
static STATE: OnceLock<Arc<Mutex<NowPlayingState>>> = OnceLock::new();

fn state() -> &'static Arc<Mutex<NowPlayingState>> {
    STATE.get_or_init(|| Arc::new(Mutex::new(NowPlayingState::fresh())))
}

// ── Event sink (mirrors Windows / macOS / Linux) ───────────────────
type EventSink = Arc<Mutex<Option<Box<dyn Fn(MediaControlEvent) + Send>>>>;
static EVENT_SINK: OnceLock<EventSink> = OnceLock::new();

fn event_sink() -> &'static EventSink {
    EVENT_SINK.get_or_init(|| Arc::new(Mutex::new(None)))
}

pub fn set_event_handler(handler: Box<dyn Fn(MediaControlEvent) + Send>) {
    let sink = event_sink();
    if let Ok(mut guard) = sink.lock() {
        *guard = Some(handler);
    }
}

fn emit(event_type: MediaControlEventType) {
    let sink = event_sink();
    if let Ok(guard) = sink.lock() {
        if let Some(handler) = guard.as_ref() {
            handler(MediaControlEvent { event_type });
        }
    }
}

// ── Controller ─────────────────────────────────────────────────────
pub struct AndroidController;

impl AndroidController {
    pub fn new() -> Self {
        Self
    }
}

fn poisoned() -> Box<dyn StdError + Send> {
    Box::new(std::io::Error::other("now-playing state mutex poisoned"))
}

/// Minimal percent-decoder for unpacking `http://asset.localhost/<encoded>`
/// back to a real filesystem path. Avoids pulling in a URL-encoding crate
/// just for this single use site. Invalid escapes are passed through.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (h1, h2) {
                out.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

impl MediaController for AndroidController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        // Service lifecycle is handled by Kotlin (`SystemMediaPlugin.init` calls
        // `MediaPlaybackService.ensureRunning`). Nothing to do here.
        Ok(())
    }

    fn set_metadata(&mut self, meta: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> {
        {
            let mut s = state().lock().map_err(|_| poisoned())?;
            s.title = meta.title.clone();
            s.artist = meta.artist.clone().unwrap_or_default();
            s.album = meta.album.clone().unwrap_or_default();
            s.duration_ms = meta
                .duration
                .map(|d| (d * 1000.0).max(0.0) as i64)
                .unwrap_or(0);

        // Split artwork URL by scheme so the Kotlin side picks the right loader.
        //
        // The frontend's `getCoverUrl` returns Tauri's webview-only custom
        // protocol `http://asset.localhost/<percent-encoded-path>`. The
        // MediaPlaybackService runs outside the WebView and can't fetch that —
        // we unpack the encoded path here and pass it as `artwork_path` so
        // Kotlin can BitmapFactory.decodeFile it directly.
        //
        //   http://asset.localhost/<encoded> → artwork_path
        //   http(s)://<any other host>       → artwork_url (HttpURLConnection)
        //   file://<path>                    → artwork_path
        //   bare path                        → artwork_path
        let aw = meta.artwork_url.clone().unwrap_or_default();
        if let Some(rest) = aw.strip_prefix("http://asset.localhost/") {
            s.artwork_path = percent_decode(rest);
            s.artwork_url.clear();
        } else if aw.starts_with("http://") || aw.starts_with("https://") {
            s.artwork_url = aw;
            s.artwork_path.clear();
        } else if let Some(stripped) = aw.strip_prefix("file://") {
            s.artwork_path = stripped.to_string();
            s.artwork_url.clear();
        } else if !aw.is_empty() {
            s.artwork_path = aw;
            s.artwork_url.clear();
        } else {
            s.artwork_path.clear();
            s.artwork_url.clear();
        }

        // Service uses `id` for track-change detection. The cross-platform
        // MediaMetadata struct has no stable id, so derive one from
        // title+artist. Same track → same id → service only re-applies
        // metadata when title/artist or the artwork URL actually changes.
        s.id = if s.title.is_empty() {
            String::new()
        } else {
            format!("{}::{}", s.title, s.artist)
        };
        }
        notify_update();
        Ok(())
    }

    fn set_playback_status(&mut self, status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> {
        {
            let mut s = state().lock().map_err(|_| poisoned())?;
            s.is_playing = matches!(status, PlaybackStatus::Playing);
        }
        notify_update();
        Ok(())
    }

    fn set_position(&mut self, position_secs: f64) -> Result<(), Box<dyn StdError + Send>> {
        {
            let mut s = state().lock().map_err(|_| poisoned())?;
            s.position_ms = (position_secs * 1000.0).max(0.0) as i64;
        }
        notify_update();
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        {
            let mut s = state().lock().map_err(|_| poisoned())?;
            *s = NowPlayingState::fresh();
        }
        notify_update();
        Ok(())
    }
}

// ── JNI exports ────────────────────────────────────────────────────
//
// These are called from the Kotlin `SystemMediaBridge` companion object.
// Naming follows JNI mangling: `Java_<pkg with _ separators>_<Class>_<method>`.

use jni::objects::{JClass, JObject};
use jni::sys::{jlong, jstring};
use jni::JNIEnv;

fn build_now_playing_json() -> String {
    let snapshot = match state().lock() {
        Ok(g) => g.clone(),
        Err(_) => return "{}".to_string(),
    };
    serde_json::json!({
        "id": snapshot.id,
        "title": snapshot.title,
        "artist": snapshot.artist,
        "album": snapshot.album,
        "artwork_path": snapshot.artwork_path,
        "artwork_url": snapshot.artwork_url,
        "is_playing": snapshot.is_playing,
        "position_ms": snapshot.position_ms,
        "duration_ms": snapshot.duration_ms,
        "can_next": snapshot.can_next,
        "can_previous": snapshot.can_previous,
    })
    .to_string()
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeGetNowPlayingJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let json = build_now_playing_json();
    match env.new_string(json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePlay(
    _env: JNIEnv,
    _class: JClass,
) {
    emit(MediaControlEventType::Play);
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePause(
    _env: JNIEnv,
    _class: JClass,
) {
    emit(MediaControlEventType::Pause);
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeResume(
    _env: JNIEnv,
    _class: JClass,
) {
    // Treated as a play request — JS side decides whether it's a true resume.
    emit(MediaControlEventType::Play);
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeTogglePlayPause(
    _env: JNIEnv,
    _class: JClass,
) {
    emit(MediaControlEventType::PlayPause);
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeNext(
    _env: JNIEnv,
    _class: JClass,
) {
    emit(MediaControlEventType::Next);
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePrevious(
    _env: JNIEnv,
    _class: JClass,
) {
    emit(MediaControlEventType::Previous);
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeSeekToMs(
    _env: JNIEnv,
    _class: JClass,
    position_ms: jlong,
) {
    emit(MediaControlEventType::SeekTo(position_ms.max(0) as f64 / 1000.0));
}

#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeRefreshOutput(
    _env: JNIEnv,
    _class: JClass,
) {
    // No-op: the audio engine watches device routing on its own (cpal).
    // Kept so the Kotlin side's declared `external fun` resolves.
}

/// Register (or clear, when `callback` is null) a `java.lang.Runnable` that
/// Rust invokes whenever the cached now-playing state changes. The Service
/// uses this to refresh the MediaSession in real time instead of waiting
/// for the next 5 s safety poll.
#[no_mangle]
pub extern "system" fn Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeSetUpdateCallback<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    callback: JObject<'local>,
) {
    // Capture the JavaVM on first call so future Rust-thread callbacks can
    // re-attach without a JNIEnv parameter.
    if JAVA_VM.get().is_none() {
        if let Ok(vm) = env.get_java_vm() {
            let _ = JAVA_VM.set(vm);
        }
    }

    let mut guard = match callback_slot().lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    if callback.as_raw().is_null() {
        *guard = None;
        return;
    }

    // Hand-off to a global ref so the JVM keeps the Runnable alive past
    // this JNI frame.
    match env.new_global_ref(&callback) {
        Ok(gref) => *guard = Some(gref),
        Err(e) => log::warn!("[system-media] new_global_ref(callback) failed: {:?}", e),
    }
}

/// Touch every JNI export so the cdylib linker keeps them. Must be called
/// at startup from a reachable code path (see `bayin_system_media::init`).
pub fn ensure_jni_exports_linked() {
    let _ = [
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeGetNowPlayingJson as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePlay as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePause as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeResume as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeTogglePlayPause as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeNext as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativePrevious as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeSeekToMs as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeRefreshOutput as usize,
        Java_app_tauri_bayin_systemmedia_SystemMediaBridge_nativeSetUpdateCallback as usize,
    ];
}
