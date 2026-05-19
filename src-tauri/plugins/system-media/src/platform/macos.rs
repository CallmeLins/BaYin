//! macOS implementation using MPNowPlayingInfoCenter and MPRemoteCommandCenter.
//!
//! NOTE: MPNowPlayingInfoCenter and MPRemoteCommandCenter MUST be called from
//! the main thread. Since Tauri commands run on a worker pool, all system-media
//! API calls are dispatched to the main thread via GCD (`dispatch_sync_f`).

#![allow(unexpected_cfgs)]

use super::MediaController;
use crate::models::*;
use std::error::Error as StdError;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use block::ConcreteBlock;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSArray, NSDictionary, NSString};
use objc::{class, msg_send, declare::ClassDecl, runtime::{Object, Sel}, sel, sel_impl};

// Needed for MPMediaItemArtwork.initWithBoundsSize:requestHandler:
#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}

// ── MediaPlayer framework constants ────────────────────────────────
//
// CRITICAL: The NowPlayingInfo dictionary keys are NSString constants exposed
// by MediaPlayer.framework. Their *runtime values* are short identifiers like
// "title", "artist", "playbackDuration", "artwork" — NOT their symbol names.
// Earlier this file used string literals like "MPMediaItemPropertyTitle" as
// keys, which silently mismatch the framework's expected keys, so Control
// Center sees an empty dictionary and shows no progress / no artwork.
//
// We declare the framework symbols as `usize` (just the underlying pointer
// bits) and cast to `id` when constructing the dictionary. `usize` sidesteps
// `Sync`/`Sized` constraints that come with declaring a `*const Object` or
// `&'static Object` extern static.
#[allow(non_upper_case_globals)]
extern "C" {
    static MPMediaItemPropertyTitle: usize;
    static MPMediaItemPropertyArtist: usize;
    static MPMediaItemPropertyAlbumTitle: usize;
    static MPMediaItemPropertyPlaybackDuration: usize;
    static MPMediaItemPropertyArtwork: usize;
    static MPNowPlayingInfoPropertyElapsedPlaybackTime: usize;
    static MPNowPlayingInfoPropertyPlaybackRate: usize;
    static MPNowPlayingInfoPropertyMediaType: usize;
}

// ── GCD main-thread dispatch ───────────────────────────────────────

type DispatchQueueT = *mut c_void;
type DispatchFn = unsafe extern "C" fn(context: *mut c_void);

// dispatch_get_main_queue() is an inline function on macOS that returns &_dispatch_main_q.
// We reference the global directly to avoid linking issues.
extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_sync_f(queue: DispatchQueueT, context: *mut c_void, work: DispatchFn);
}

fn get_main_queue() -> DispatchQueueT {
    // Address of the _dispatch_main_q global IS the main dispatch queue.
    unsafe { &_dispatch_main_q as *const c_void as DispatchQueueT }
}

/// Check if we're currently on the main thread (using NSThread.isMainThread).
fn is_main_thread() -> bool {
    unsafe { msg_send![class!(NSThread), isMainThread] }
}

/// Run a closure synchronously on the main thread via Grand Central Dispatch.
/// Required because MPRemoteCommandCenter / MPNowPlayingInfoCenter are main-thread-only APIs.
///
/// If already on the main thread, runs the closure directly to avoid
/// the "dispatch_sync called on queue already owned by current thread" crash.
fn run_on_main_sync<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    // Fast path: already on main thread → run directly
    if is_main_thread() {
        return f();
    }

    unsafe {
        let mut result: Option<R> = None;
        // Cast through usize to avoid raw pointer in the closure (raw pointers are !Send)
        let result_ptr_usize = &mut result as *mut Option<R> as usize;

        // Double-box for type erasure: closure → Box<dyn FnOnce> → Box<Box<dyn FnOnce>>
        let closure: Box<dyn FnOnce() + Send> = Box::new(move || {
            let result_ptr = result_ptr_usize as *mut Option<R>;
            *result_ptr = Some(f());
        });
        let ctx = Box::into_raw(Box::new(closure)) as *mut c_void;

        unsafe extern "C" fn trampoline(ctx: *mut c_void) {
            let f: Box<Box<dyn FnOnce() + Send>> =
                Box::from_raw(ctx as *mut Box<dyn FnOnce() + Send>);
            f();
        }

        dispatch_sync_f(get_main_queue(), ctx, trampoline);
        result.take().unwrap()
    }
}

// ── Global event sink ──────────────────────────────────────────────
type EventSink = Arc<Mutex<Option<Box<dyn Fn(MediaControlEvent) + Send>>>>;
static EVENT_SINK: OnceLock<EventSink> = OnceLock::new();

fn event_sink() -> &'static EventSink {
    EVENT_SINK.get_or_init(|| Arc::new(Mutex::new(None)))
}

/// Called from the frontend to register the event callback.
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

// ── ObjC helper class ──────────────────────────────────────────────
const HELPER_CLASS: &str = "BaYinMediaRemoteHandler";

fn ensure_helper_class() -> *const objc::runtime::Class {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new(HELPER_CLASS, superclass)
            .expect("Failed to register BaYinMediaRemoteHandler");

        // All handlers must return MPRemoteCommandHandlerStatus (NSInteger = isize).
        // macOS 15+ enforces this: returning void crashes with NSInternalInconsistencyException.
        const SUCCESS: isize = 0; // MPRemoteCommandHandlerStatusSuccess

        // emit() is wrapped in catch_unwind because these are extern "C" functions
        // that cannot unwind. Any panic in emit() would otherwise abort the process.
        macro_rules! safe_emit {
            ($event:expr) => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    emit($event);
                }));
            };
        }

        extern "C" fn on_play(_this: &Object, _sel: Sel, _event: id) -> isize {
            safe_emit!(MediaControlEventType::Play);
            SUCCESS
        }
        extern "C" fn on_pause(_this: &Object, _sel: Sel, _event: id) -> isize {
            safe_emit!(MediaControlEventType::Pause);
            SUCCESS
        }
        extern "C" fn on_stop(_this: &Object, _sel: Sel, _event: id) -> isize {
            safe_emit!(MediaControlEventType::Stop);
            SUCCESS
        }
        extern "C" fn on_next(_this: &Object, _sel: Sel, _event: id) -> isize {
            safe_emit!(MediaControlEventType::Next);
            SUCCESS
        }
        extern "C" fn on_prev(_this: &Object, _sel: Sel, _event: id) -> isize {
            safe_emit!(MediaControlEventType::Previous);
            SUCCESS
        }
        extern "C" fn on_toggle(_this: &Object, _sel: Sel, _event: id) -> isize {
            safe_emit!(MediaControlEventType::PlayPause);
            SUCCESS
        }
        extern "C" fn on_seek(_this: &Object, _sel: Sel, event: id) -> isize {
            unsafe {
                // MPChangePlaybackPositionCommandEvent.positionTime returns NSTimeInterval (double) directly
                let pos: f64 = msg_send![event, positionTime];
                safe_emit!(MediaControlEventType::SeekTo(pos));
            }
            SUCCESS
        }

        unsafe {
            decl.add_method(sel!(handlePlay:), on_play as extern "C" fn(&Object, Sel, id) -> isize);
            decl.add_method(sel!(handlePause:), on_pause as extern "C" fn(&Object, Sel, id) -> isize);
            decl.add_method(sel!(handleStop:), on_stop as extern "C" fn(&Object, Sel, id) -> isize);
            decl.add_method(sel!(handleNext:), on_next as extern "C" fn(&Object, Sel, id) -> isize);
            decl.add_method(sel!(handlePrev:), on_prev as extern "C" fn(&Object, Sel, id) -> isize);
            decl.add_method(sel!(handleToggle:), on_toggle as extern "C" fn(&Object, Sel, id) -> isize);
            decl.add_method(sel!(handleSeek:), on_seek as extern "C" fn(&Object, Sel, id) -> isize);
        }
        decl.register();
    });
    class!(BaYinMediaRemoteHandler)
}

// ── Controller ─────────────────────────────────────────────────────

struct ObjcId(id);
unsafe impl Send for ObjcId {}

pub struct MacOsController {
    handler: ObjcId,
    initialized: bool,
}

impl MacOsController {
    pub fn new() -> Self {
        let handler = unsafe {
            let cls = ensure_helper_class();
            ObjcId(msg_send![cls, new])
        };
        MacOsController { handler, initialized: false }
    }
}

impl Drop for MacOsController {
    fn drop(&mut self) {
        unsafe {
            if !self.handler.0.is_null() {
                let _: () = msg_send![self.handler.0, release];
            }
        }
    }
}

impl MediaController for MacOsController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        if self.initialized {
            return Ok(());
        }
        let handler_usize = self.handler.0 as usize;
        let result = run_on_main_sync(move || -> Result<(), Box<dyn StdError + Send>> {
            let target = handler_usize as id;
            unsafe {
                let cc: id = msg_send![class!(MPRemoteCommandCenter), sharedCommandCenter];

                let commands: &[(id, Sel)] = &[
                    (msg_send![cc, playCommand], sel!(handlePlay:)),
                    (msg_send![cc, pauseCommand], sel!(handlePause:)),
                    (msg_send![cc, stopCommand], sel!(handleStop:)),
                    (msg_send![cc, nextTrackCommand], sel!(handleNext:)),
                    (msg_send![cc, previousTrackCommand], sel!(handlePrev:)),
                    (msg_send![cc, togglePlayPauseCommand], sel!(handleToggle:)),
                    (msg_send![cc, changePlaybackPositionCommand], sel!(handleSeek:)),
                ];

                for &(cmd, action) in commands {
                    let _: () = msg_send![cmd, setEnabled: true];
                    let _: () = msg_send![cmd, addTarget:target action:action];
                }
            }
            Ok(())
        });
        result?;
        self.initialized = true;
        log::info!("[system-media] macOS initialized, commands registered");
        Ok(())
    }

    fn set_metadata(&mut self, meta: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> {
        let title = meta.title.clone();
        let artist = meta.artist.clone();
        let album = meta.album.clone();
        let duration = meta.duration;
        let artwork_url = meta.artwork_url.clone();
        let log_title = title.clone();
        run_on_main_sync(move || -> Result<(), Box<dyn StdError + Send>> {
            unsafe {
                let ic: id = msg_send![class!(MPNowPlayingInfoCenter), defaultCenter];
                let mut keys: Vec<id> = Vec::new();
                let mut vals: Vec<id> = Vec::new();

                keys.push(MPMediaItemPropertyTitle as id);
                vals.push(NSString::alloc(nil).init_str(&title));

                if let Some(ref artist) = artist {
                    keys.push(MPMediaItemPropertyArtist as id);
                    vals.push(NSString::alloc(nil).init_str(artist));
                }
                if let Some(ref album) = album {
                    keys.push(MPMediaItemPropertyAlbumTitle as id);
                    vals.push(NSString::alloc(nil).init_str(album));
                }
                if let Some(duration) = duration {
                    keys.push(MPMediaItemPropertyPlaybackDuration as id);
                    vals.push(msg_send![class!(NSNumber), numberWithDouble: duration]);
                }

                // ── Artwork ──────────────────────────────────────
                // Accepts plain file paths, file:// URLs, and http(s):// URLs.
                // Loads via NSData so HTTP works (synchronously on the main
                // thread — fine for a one-off cover URL, and matches what we
                // need to construct MPMediaItemArtwork up front).
                if let Some(ref url_str) = artwork_url {
                    let has_scheme = url_str.starts_with("file://")
                        || url_str.starts_with("http://")
                        || url_str.starts_with("https://");
                    let ns_str = NSString::alloc(nil).init_str(url_str);
                    let ns_url: id = if has_scheme {
                        msg_send![class!(NSURL), URLWithString: ns_str]
                    } else {
                        // Bare path: percent-encoding etc. handled by fileURLWithPath:.
                        msg_send![class!(NSURL), fileURLWithPath: ns_str]
                    };
                    if !ns_url.is_null() {
                        let data: id = msg_send![class!(NSData), dataWithContentsOfURL: ns_url];
                        if !data.is_null() {
                            let img: id = msg_send![class!(NSImage), alloc];
                            let img: id = msg_send![img, initWithData: data];
                            if !img.is_null() {
                                let _: () = msg_send![img, retain];
                                let size = CGSize { width: 600.0, height: 600.0 };
                                let img_ref = img;
                                let handler = ConcreteBlock::new(move |_: CGSize| -> id {
                                    img_ref
                                });
                                let handler = handler.copy();
                                let artwork: id = msg_send![class!(MPMediaItemArtwork), alloc];
                                let artwork: id = msg_send![artwork, initWithBoundsSize: size requestHandler: &*handler];
                                if !artwork.is_null() {
                                    keys.push(MPMediaItemPropertyArtwork as id);
                                    vals.push(artwork);
                                } else {
                                    log::warn!("[system-media] MPMediaItemArtwork init failed");
                                }
                            } else {
                                log::warn!("[system-media] NSImage initWithData failed for '{}'", url_str);
                            }
                        } else {
                            log::warn!("[system-media] NSData dataWithContentsOfURL failed for '{}'", url_str);
                        }
                    } else {
                        log::warn!("[system-media] NSURL creation failed for '{}'", url_str);
                    }
                }

                // MPNowPlayingInfoMediaTypeAudio = 1 — tells Control Center
                // this is an audio session, which affects the layout it picks.
                keys.push(MPNowPlayingInfoPropertyMediaType as id);
                vals.push(msg_send![class!(NSNumber), numberWithUnsignedInt: 1u32]);

                // ── Initial playback state ───────────────────────
                // Rate=0/elapsed=0 here means "paused at start"; callers are
                // expected to follow up with set_playback_status(Playing) and
                // optionally set_position(...) to set the actual progress.
                keys.push(MPNowPlayingInfoPropertyPlaybackRate as id);
                vals.push(msg_send![class!(NSNumber), numberWithDouble: 0.0f64]);
                keys.push(MPNowPlayingInfoPropertyElapsedPlaybackTime as id);
                vals.push(msg_send![class!(NSNumber), numberWithDouble: 0.0f64]);

                let k_arr = NSArray::arrayWithObjects(nil, &keys);
                let v_arr = NSArray::arrayWithObjects(nil, &vals);
                let dict = NSDictionary::dictionaryWithObjects_forKeys_(nil, v_arr, k_arr);
                let _: () = msg_send![ic, setNowPlayingInfo: dict];
            }
            log::info!("[system-media] metadata set: {}", log_title);
            Ok(())
        })
    }

    fn set_playback_status(&mut self, status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> {
        run_on_main_sync(move || -> Result<(), Box<dyn StdError + Send>> {
            unsafe {
                let ic: id = msg_send![class!(MPNowPlayingInfoCenter), defaultCenter];
                let current: id = msg_send![ic, nowPlayingInfo];
                if !current.is_null() {
                    let dict: id = msg_send![current, mutableCopy];
                    let key: id = MPNowPlayingInfoPropertyPlaybackRate as id;
                    let rate: f64 = if status == PlaybackStatus::Playing { 1.0 } else { 0.0 };
                    let val: id = msg_send![class!(NSNumber), numberWithDouble: rate];
                    let _: () = msg_send![dict, setObject:val forKey:key];
                    let _: () = msg_send![ic, setNowPlayingInfo: dict];
                }
            }
            Ok(())
        })
    }

    fn set_position(&mut self, position_secs: f64) -> Result<(), Box<dyn StdError + Send>> {
        run_on_main_sync(move || -> Result<(), Box<dyn StdError + Send>> {
            unsafe {
                let ic: id = msg_send![class!(MPNowPlayingInfoCenter), defaultCenter];
                let current: id = msg_send![ic, nowPlayingInfo];
                if !current.is_null() {
                    let dict: id = msg_send![current, mutableCopy];
                    let key: id = MPNowPlayingInfoPropertyElapsedPlaybackTime as id;
                    let val: id = msg_send![class!(NSNumber), numberWithDouble: position_secs];
                    let _: () = msg_send![dict, setObject:val forKey:key];
                    let _: () = msg_send![ic, setNowPlayingInfo: dict];
                }
            }
            Ok(())
        })
    }

    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        run_on_main_sync(move || -> Result<(), Box<dyn StdError + Send>> {
            unsafe {
                let ic: id = msg_send![class!(MPNowPlayingInfoCenter), defaultCenter];
                let _: () = msg_send![ic, setNowPlayingInfo: nil];
            }
            Ok(())
        })
    }
}
