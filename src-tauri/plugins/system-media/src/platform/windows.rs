//! Windows implementation using System Media Transport Controls (SMTC).

use super::MediaController;
use crate::models::*;
use std::error::Error as StdError;
use std::sync::{Arc, Mutex, OnceLock};

use windows::core::HSTRING;
use windows::Foundation::TimeSpan;
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
};
use windows::Media::Playback::MediaPlayer;
use windows::Storage::Streams::{InMemoryRandomAccessStream, RandomAccessStreamReference};
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

pub struct WindowsController {
    player: Option<MediaPlayer>,
    stc: Option<SystemMediaTransportControls>,
    initialized: bool,
}

unsafe impl Send for WindowsController {}

impl WindowsController {
    pub fn new() -> Self {
        WindowsController {
            player: None,
            stc: None,
            initialized: false,
        }
    }
}

impl MediaController for WindowsController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        unsafe { let _ = RoInitialize(RO_INIT_MULTITHREADED); }

        let player = MediaPlayer::new()?;
        player.set_auto_play(false)?;
        player.set_is_muted(true)?;

        let stc = player.system_media_transport_controls()?;
        stc.set_is_enabled(true)?;
        stc.set_playback_status(MediaPlaybackStatus::Playing)?;

        // Register button handler
        let stc_clone = stc.clone();
        stc.button_pressed(TypedEventHandler::new(
            move |_sender: &Option<SystemMediaTransportControls>,
                  args: &Option<SystemMediaTransportControlsButtonPressedEventArgs>| {
                if let Some(args) = args {
                    if let Ok(button) = args.button() {
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
        ))?;

        self.player = Some(player);
        self.stc = Some(stc);
        self.initialized = true;
        Ok(())
    }

    fn set_metadata(&mut self, meta: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> {
        let stc = self.stc.as_ref().ok_or("SMTC not initialized")?;
        let updater = stc.display_updater()?;

        updater.set_type(MediaPlaybackType::Music)?;
        let music_props = updater.music_properties()?;
        music_props.set_title(&HSTRING::from(&meta.title))?;
        if let Some(ref artist) = meta.artist {
            music_props.set_artist(&HSTRING::from(artist))?;
        }
        if let Some(ref album) = meta.album {
            music_props.set_album_title(&HSTRING::from(album))?;
        }

        // Set duration in timeline
        if let Some(duration) = meta.duration {
            let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
            timeline.set_start_time(TimeSpan { Duration: 0 })?;
            timeline.set_end_time(to_timespan(duration))?;
            timeline.set_position(to_timespan(0.0))?;
            stc.update_timeline_properties(timeline)?;
        }

        updater.update()?;
        Ok(())
    }

    fn set_playback_status(&mut self, status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> {
        let stc = self.stc.as_ref().ok_or("SMTC not initialized")?;
        let s = match status {
            PlaybackStatus::Playing => MediaPlaybackStatus::Playing,
            PlaybackStatus::Paused => MediaPlaybackStatus::Paused,
            PlaybackStatus::Stopped => MediaPlaybackStatus::Stopped,
        };
        stc.set_playback_status(s)?;
        Ok(())
    }

    fn set_position(&mut self, position_secs: f64) -> Result<(), Box<dyn StdError + Send>> {
        let stc = self.stc.as_ref().ok_or("SMTC not initialized")?;
        let timeline = stc.timeline_properties()?;
        timeline.set_position(to_timespan(position_secs))?;
        stc.update_timeline_properties(timeline)?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        let stc = self.stc.as_ref().ok_or("SMTC not initialized")?;
        let updater = stc.display_updater()?;
        updater.clear_all()?;
        updater.update()?;
        Ok(())
    }
}

use windows::Media::SystemMediaTransportControlsButtonPressedEventArgs;
use windows::Media::SystemMediaTransportControlsTimelineProperties;
use windows::Foundation::TypedEventHandler;
