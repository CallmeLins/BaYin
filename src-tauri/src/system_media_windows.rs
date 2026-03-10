//! Windows system media integration (System Media Transport Controls).
//!
//! This integrates BaYin's custom audio engine with Windows' media keys / volume flyout
//! by publishing metadata + playback status + artwork, and by handling transport callbacks.

#![cfg(target_os = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::Manager;
use tauri::Emitter;

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
use crate::commands::CoverCacheState;
use crate::playback::PlaybackDomainState;
use crate::utils::cover::CoverSize;

use windows::core::{HSTRING, Ref, Result as WinResult};
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Media::Playback::MediaPlayer;
use windows::Storage::StorageFile;
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

#[derive(Debug, Clone)]
struct NowPlayingSnapshot {
    id: String,
    title: String,
    artist: String,
    album: String,
    duration_secs: f64,
    artwork_hash: Option<String>,
    artwork_url: Option<String>,
    is_playing: bool,
    position_secs: f64,
    can_next: bool,
    can_previous: bool,
}

fn snapshot(app: &tauri::AppHandle) -> Option<NowPlayingSnapshot> {
    let domain = app.try_state::<PlaybackDomainState>()?;
    let engine = app.try_state::<AudioEngineState>()?;

    let (track, queue_len) = {
        let d = domain.0.lock().ok()?;
        let t = d.queue.get(d.index)?.clone();
        (t, d.queue.len())
    };

    let state = {
        let engine = engine.lock().ok()?;
        let cloned = engine.state.lock().ok()?.clone();
        cloned
    };

    Some(NowPlayingSnapshot {
        id: track.id,
        title: track.title,
        artist: track.artist,
        album: track.album,
        duration_secs: if track.duration_secs > 0.0 {
            track.duration_secs
        } else {
            state.duration_secs
        },
        artwork_hash: track.artwork_ref,
        artwork_url: track.artwork_url,
        is_playing: state.is_playing,
        position_secs: state.position_secs.max(0.0),
        can_next: queue_len > 1,
        can_previous: queue_len > 1,
    })
}

fn set_thumbnail_from_cover_hash(
    app: &tauri::AppHandle,
    updater: &windows::Media::SystemMediaTransportControlsDisplayUpdater,
    cover_hash: &str,
) -> WinResult<()> {
    let cache = match app.try_state::<CoverCacheState>() {
        Some(s) => s,
        None => return Ok(()),
    };
    let cache = match cache.0.lock() {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let path = cache
        .get_cover_path(cover_hash, CoverSize::Mid)
        .ok_or_else(windows::core::Error::from_win32)?;

    let path_str = path.to_string_lossy().to_string();
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path_str))?.get()?;
    let thumb = RandomAccessStreamReference::CreateFromFile(&file)?;
    updater.SetThumbnail(Some(&thumb))?;
    Ok(())
}

fn set_thumbnail_from_url(
    updater: &windows::Media::SystemMediaTransportControlsDisplayUpdater,
    url: &str,
) -> WinResult<()> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let thumb = RandomAccessStreamReference::CreateFromUri(&uri)?;
    updater.SetThumbnail(Some(&thumb))?;
    Ok(())
}

fn update_display(
    app: &tauri::AppHandle,
    smtc: &SystemMediaTransportControls,
    updater: &windows::Media::SystemMediaTransportControlsDisplayUpdater,
    snap: &NowPlayingSnapshot,
    last_track_id: &mut String,
    last_artwork_key: &mut String,
) -> WinResult<()> {
    // Playback buttons + status
    smtc.SetIsEnabled(true)?;
    smtc.SetIsPlayEnabled(true)?;
    smtc.SetIsPauseEnabled(true)?;
    smtc.SetIsStopEnabled(true)?;
    smtc.SetIsNextEnabled(snap.can_next)?;
    smtc.SetIsPreviousEnabled(snap.can_previous)?;

    let status = if snap.is_playing {
        MediaPlaybackStatus::Playing
    } else {
        MediaPlaybackStatus::Paused
    };
    smtc.SetPlaybackStatus(status)?;

    // Timeline
    let dur_100ns = (snap.duration_secs.max(0.0) * 10_000_000.0).round() as i64;
    let pos_100ns = (snap.position_secs.max(0.0) * 10_000_000.0).round() as i64;
    let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
    timeline.SetStartTime(TimeSpan { Duration: 0 })?;
    timeline.SetMinSeekTime(TimeSpan { Duration: 0 })?;
    timeline.SetEndTime(TimeSpan {
        Duration: dur_100ns.max(0),
    })?;
    timeline.SetMaxSeekTime(TimeSpan {
        Duration: dur_100ns.max(0),
    })?;
    timeline.SetPosition(TimeSpan {
        Duration: pos_100ns.clamp(0, dur_100ns.max(0)),
    })?;
    smtc.UpdateTimelineProperties(&timeline)?;

    // Metadata
    let track_changed = *last_track_id != snap.id;
    if track_changed {
        updater.SetType(MediaPlaybackType::Music)?;
        let music = updater.MusicProperties()?;
        music.SetTitle(&HSTRING::from(snap.title.as_str()))?;
        music.SetArtist(&HSTRING::from(snap.artist.as_str()))?;
        music.SetAlbumTitle(&HSTRING::from(snap.album.as_str()))?;
        *last_track_id = snap.id.clone();
    }

    // Artwork can become available later (lazy caching), so track it separately.
    let artwork_key = snap
        .artwork_hash
        .as_deref()
        .map(|s| format!("hash:{}", s))
        .or_else(|| snap.artwork_url.as_deref().map(|s| format!("url:{}", s)))
        .unwrap_or_default();
    if *last_artwork_key != artwork_key {
        updater.SetThumbnail(None)?;
        if let Some(hash) = snap.artwork_hash.as_deref() {
            let _ = set_thumbnail_from_cover_hash(app, updater, hash);
        } else if let Some(url) = snap.artwork_url.as_deref() {
            let _ = set_thumbnail_from_url(updater, url);
        }
        *last_artwork_key = artwork_key.clone();
    }

    // Always update when track changed or artwork key changed.
    if track_changed || *last_artwork_key == artwork_key {
        updater.Update()?;
    }

    Ok(())
}

fn handle_button(app: &tauri::AppHandle, button: SystemMediaTransportControlsButton) {
    let domain = match app.try_state::<PlaybackDomainState>() {
        Some(s) => s,
        None => return,
    };
    let engine = match app.try_state::<AudioEngineState>() {
        Some(s) => s,
        None => return,
    };

    match button {
        SystemMediaTransportControlsButton::Play => {
            let (is_playing, has_loaded_source) = engine
                .lock()
                .ok()
                .and_then(|e| {
                    e.state
                        .lock()
                        .ok()
                        .map(|s| (s.is_playing, s.duration_secs > 0.0))
                })
                .unwrap_or((false, false));
            if is_playing {
                return;
            }

            if has_loaded_source {
                if let Ok(engine) = engine.lock() {
                    engine.send(AudioCommand::Resume);
                }
                return;
            }

            let idx = { domain.0.lock().ok().map(|d| d.index).unwrap_or(0) };
            if crate::playback_control::play_index(idx, &domain, &engine) {
                emit_domain_changed(app);
            }
        }
        SystemMediaTransportControlsButton::Pause => {
            if let Ok(engine) = engine.lock() {
                engine.send(AudioCommand::Pause);
            }
        }
        SystemMediaTransportControlsButton::Stop => {
            if let Ok(engine) = engine.lock() {
                engine.send(AudioCommand::Stop);
            }
        }
        SystemMediaTransportControlsButton::Next => {
            if crate::playback_control::next(&domain, &engine) {
                emit_domain_changed(app);
            }
        }
        SystemMediaTransportControlsButton::Previous => {
            if crate::playback_control::previous(&domain, &engine) {
                emit_domain_changed(app);
            }
        }
        _ => {}
    }
}

fn emit_domain_changed(app: &tauri::AppHandle) {
    let domain = match app.try_state::<PlaybackDomainState>() {
        Some(s) => s,
        None => return,
    };
    let (index, track_id) = {
        let d = match domain.0.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        if d.queue.is_empty() || d.index >= d.queue.len() {
            return;
        }
        (d.index, d.queue[d.index].id.clone())
    };
    let _ = app.emit(
        "playback:domain_changed",
        serde_json::json!({ "index": index, "track_id": track_id }),
    );
}

pub fn init(app: tauri::AppHandle) {
    // Allow turning this off in the future without invasive refactors.
    let alive = Arc::new(AtomicBool::new(true));
    let alive_thread = alive.clone();
    let app_for_thread = app.clone();

    std::thread::spawn(move || {
        // WinRT APIs require apartment initialization on this thread.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }

        let player = match MediaPlayer::new() {
            Ok(p) => p,
            Err(_) => return,
        };

        let smtc = match player.SystemMediaTransportControls() {
            Ok(s) => s,
            Err(_) => return,
        };

        // Publish a session even if paused, so metadata is visible as soon as a track is selected.
        let updater = match smtc.DisplayUpdater() {
            Ok(u) => u,
            Err(_) => return,
        };

        smtc.SetIsEnabled(true).ok();
        smtc.SetIsPlayEnabled(true).ok();
        smtc.SetIsPauseEnabled(true).ok();
        smtc.SetIsStopEnabled(true).ok();
        smtc.SetIsNextEnabled(true).ok();
        smtc.SetIsPreviousEnabled(true).ok();
        smtc.SetPlaybackStatus(MediaPlaybackStatus::Stopped).ok();

        // Transport callbacks
        {
            let app = app_for_thread.clone();
            let handler: TypedEventHandler<SystemMediaTransportControls, SystemMediaTransportControlsButtonPressedEventArgs> =
                TypedEventHandler::new(move |_sender: Ref<'_, SystemMediaTransportControls>, args: Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
                    let args = args.ok()?;
                    handle_button(&app, args.Button()?);
                    Ok(())
                });
            let _ = smtc.ButtonPressed(&handler);
        }

        // Seek callbacks
        {
            let app = app_for_thread.clone();
            let handler: TypedEventHandler<SystemMediaTransportControls, PlaybackPositionChangeRequestedEventArgs> =
                TypedEventHandler::new(move |_sender: Ref<'_, SystemMediaTransportControls>, args: Ref<'_, PlaybackPositionChangeRequestedEventArgs>| {
                    let args = args.ok()?;
                    let pos_100ns = args.RequestedPlaybackPosition()?.Duration;
                    let secs = (pos_100ns as f64) / 10_000_000.0;
                    if let Some(engine) = app.try_state::<AudioEngineState>() {
                        if let Ok(engine) = engine.lock() {
                            engine.send(AudioCommand::Seek {
                                position_secs: secs.max(0.0),
                            });
                        }
                    }
                    Ok(())
                });
            let _ = smtc.PlaybackPositionChangeRequested(&handler);
        }

        let mut last_track_id = String::new();
        let mut last_artwork_key = String::new();

        while alive_thread.load(Ordering::Relaxed) {
            if let Some(snap) = snapshot(&app_for_thread) {
                let _ = update_display(
                    &app_for_thread,
                    &smtc,
                    &updater,
                    &snap,
                    &mut last_track_id,
                    &mut last_artwork_key,
                );
            } else {
                // No queue / no state yet: keep session but show as stopped.
                let _ = smtc.SetPlaybackStatus(MediaPlaybackStatus::Stopped);
            }

            std::thread::sleep(Duration::from_millis(250));
        }
    });

    // Keep `alive` tied to app lifecycle by managing it.
    // (If we ever add a graceful shutdown hook, it can flip this flag.)
    app.manage(alive);
}
