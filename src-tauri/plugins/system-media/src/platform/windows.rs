//! Windows implementation using System Media Transport Controls (SMTC).
//!
//! Uses the `windows` crate v0.61 (PascalCase method names).
//!
//! Threading: Tauri commands run on a worker pool; WinRT requires each thread to
//! be initialized as MTA before calling WinRT methods. `ensure_mta()` is called
//! at the top of every controller method and is idempotent.

use super::MediaController;
use crate::models::*;
use std::error::Error as StdError;
use std::sync::{Arc, Mutex, OnceLock};

use windows::core::HSTRING;
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Media::Playback::MediaPlayer;
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

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

fn to_timespan(secs: f64) -> TimeSpan {
    TimeSpan { Duration: (secs * 10_000_000.0) as i64 }
}

/// Idempotent COM/WinRT MTA initialization for the current thread.
/// Tauri commands run on a worker pool, so each new worker thread must
/// be initialized before its first WinRT call.
fn ensure_mta() {
    unsafe { let _ = RoInitialize(RO_INIT_MULTITHREADED); }
}

fn win_err(e: windows::core::Error) -> Box<dyn StdError + Send> {
    Box::new(std::io::Error::other(e.message().to_string()))
}

fn not_init() -> Box<dyn StdError + Send> {
    Box::new(std::io::Error::other("SMTC not initialized"))
}

pub struct WindowsController {
    player: Option<MediaPlayer>,
    stc: Option<SystemMediaTransportControls>,
    initialized: bool,
    /// Cached duration so `set_position` can rebuild the timeline without
    /// dropping the total length (SMTC has no `TimelineProperties` getter).
    duration_secs: Option<f64>,
}

unsafe impl Send for WindowsController {}

impl WindowsController {
    pub fn new() -> Self {
        WindowsController {
            player: None,
            stc: None,
            initialized: false,
            duration_secs: None,
        }
    }

    fn stc(&self) -> Result<&SystemMediaTransportControls, Box<dyn StdError + Send>> {
        self.stc.as_ref().ok_or_else(not_init)
    }
}

impl MediaController for WindowsController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        if self.initialized {
            return Ok(());
        }
        ensure_mta();

        let player = MediaPlayer::new().map_err(win_err)?;
        player.SetAutoPlay(false).map_err(win_err)?;
        player.SetIsMuted(true).map_err(win_err)?;

        let stc = player.SystemMediaTransportControls().map_err(win_err)?;
        stc.SetIsEnabled(true).map_err(win_err)?;
        // Start in Closed; switch to Playing/Paused once metadata is set,
        // so we don't flash an empty session on the lock screen / Win+G.
        stc.SetPlaybackStatus(MediaPlaybackStatus::Closed).map_err(win_err)?;

        // Default capability set: most music players want these enabled.
        stc.SetIsPlayEnabled(true).map_err(win_err)?;
        stc.SetIsPauseEnabled(true).map_err(win_err)?;
        stc.SetIsStopEnabled(true).map_err(win_err)?;
        stc.SetIsNextEnabled(true).map_err(win_err)?;
        stc.SetIsPreviousEnabled(true).map_err(win_err)?;

        // Register button handler exactly once (this method early-returns if
        // already initialized, so no token tracking needed).
        stc.ButtonPressed(&TypedEventHandler::new(
            move |_sender: windows::core::Ref<'_, SystemMediaTransportControls>,
                  args: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
                if let Some(args) = args.as_ref() {
                    if let Ok(button) = args.Button() {
                        match button {
                            SystemMediaTransportControlsButton::Play => emit(MediaControlEventType::Play),
                            SystemMediaTransportControlsButton::Pause => emit(MediaControlEventType::Pause),
                            SystemMediaTransportControlsButton::Stop => emit(MediaControlEventType::Stop),
                            SystemMediaTransportControlsButton::Next => emit(MediaControlEventType::Next),
                            SystemMediaTransportControlsButton::Previous => emit(MediaControlEventType::Previous),
                            _ => {}
                        }
                    }
                }
                Ok(())
            },
        )).map_err(win_err)?;

        self.player = Some(player);
        self.stc = Some(stc);
        self.initialized = true;
        log::info!("[system-media] Windows SMTC initialized");
        Ok(())
    }

    fn set_metadata(&mut self, meta: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> {
        ensure_mta();

        // Cache duration first to avoid borrow conflict with the immutable
        // borrow from `self.stc()` below.
        if let Some(d) = meta.duration {
            self.duration_secs = Some(d);
        }

        let stc = self.stc()?;
        let updater = stc.DisplayUpdater().map_err(win_err)?;

        updater.SetType(MediaPlaybackType::Music).map_err(win_err)?;
        let music_props = updater.MusicProperties().map_err(win_err)?;
        music_props.SetTitle(&HSTRING::from(&meta.title)).map_err(win_err)?;
        if let Some(ref artist) = meta.artist {
            music_props.SetArtist(&HSTRING::from(artist)).map_err(win_err)?;
        }
        if let Some(ref album) = meta.album {
            music_props.SetAlbumTitle(&HSTRING::from(album)).map_err(win_err)?;
        }

        // Artwork: accept http(s):// or file:// URIs. Errors here are non-fatal —
        // metadata text should still update even if the image fails to load.
        if let Some(ref url_str) = meta.artwork_url {
            match Uri::CreateUri(&HSTRING::from(url_str)) {
                Ok(uri) => match RandomAccessStreamReference::CreateFromUri(&uri) {
                    Ok(stream_ref) => {
                        if let Err(e) = updater.SetThumbnail(&stream_ref) {
                            log::warn!("[system-media] SetThumbnail failed: {}", e.message());
                        }
                    }
                    Err(e) => log::warn!("[system-media] CreateFromUri failed: {}", e.message()),
                },
                Err(e) => log::warn!("[system-media] Uri::CreateUri failed for '{}': {}", url_str, e.message()),
            }
        }

        // Timeline: rebuild because SMTC has no getter for current timeline props.
        if let Some(duration) = meta.duration {
            let timeline = SystemMediaTransportControlsTimelineProperties::new().map_err(win_err)?;
            timeline.SetStartTime(TimeSpan { Duration: 0 }).map_err(win_err)?;
            timeline.SetEndTime(to_timespan(duration)).map_err(win_err)?;
            timeline.SetMinSeekTime(TimeSpan { Duration: 0 }).map_err(win_err)?;
            timeline.SetMaxSeekTime(to_timespan(duration)).map_err(win_err)?;
            timeline.SetPosition(to_timespan(0.0)).map_err(win_err)?;
            stc.UpdateTimelineProperties(&timeline).map_err(win_err)?;
        }

        updater.Update().map_err(win_err)?;
        Ok(())
    }

    fn set_playback_status(&mut self, status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> {
        ensure_mta();
        let stc = self.stc()?;
        let s = match status {
            PlaybackStatus::Playing => MediaPlaybackStatus::Playing,
            PlaybackStatus::Paused => MediaPlaybackStatus::Paused,
            PlaybackStatus::Stopped => MediaPlaybackStatus::Stopped,
        };
        stc.SetPlaybackStatus(s).map_err(win_err)?;
        Ok(())
    }

    fn set_position(&mut self, position_secs: f64) -> Result<(), Box<dyn StdError + Send>> {
        ensure_mta();
        let stc = self.stc()?;
        let timeline = SystemMediaTransportControlsTimelineProperties::new().map_err(win_err)?;
        let duration = self.duration_secs.unwrap_or(0.0);
        timeline.SetStartTime(TimeSpan { Duration: 0 }).map_err(win_err)?;
        timeline.SetEndTime(to_timespan(duration)).map_err(win_err)?;
        timeline.SetMinSeekTime(TimeSpan { Duration: 0 }).map_err(win_err)?;
        timeline.SetMaxSeekTime(to_timespan(duration)).map_err(win_err)?;
        timeline.SetPosition(to_timespan(position_secs)).map_err(win_err)?;
        stc.UpdateTimelineProperties(&timeline).map_err(win_err)?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        ensure_mta();
        self.duration_secs = None;
        let stc = self.stc()?;
        let updater = stc.DisplayUpdater().map_err(win_err)?;
        updater.ClearAll().map_err(win_err)?;
        updater.Update().map_err(win_err)?;
        stc.SetPlaybackStatus(MediaPlaybackStatus::Closed).map_err(win_err)?;
        Ok(())
    }
}
