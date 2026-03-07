use bayin_playback::{PlayMode, Track};
use tauri::State;

use crate::audio_engine::AudioEngineState;
use crate::playback::PlaybackDomainState;
use crate::playback_control;

#[tauri::command]
pub fn playback_set_queue(
    tracks: Vec<Track>,
    index: usize,
    mode: PlayMode,
    domain: State<'_, PlaybackDomainState>,
) -> () {
    let mut d = domain.0.lock().unwrap();
    d.queue = tracks;
    d.index = index;
    d.mode = mode;
}

#[tauri::command]
pub fn playback_set_mode(mode: PlayMode, domain: State<'_, PlaybackDomainState>) -> () {
    let mut d = domain.0.lock().unwrap();
    d.mode = mode;
}

#[tauri::command]
pub fn playback_play_index(
    index: usize,
    domain: State<'_, PlaybackDomainState>,
    engine: State<'_, AudioEngineState>,
) -> () {
    let _ = playback_control::play_index(index, &domain, &engine);
}

#[tauri::command]
pub fn playback_next(domain: State<'_, PlaybackDomainState>, engine: State<'_, AudioEngineState>) -> () {
    let _ = playback_control::next(&domain, &engine);
}

#[tauri::command]
pub fn playback_previous(
    domain: State<'_, PlaybackDomainState>,
    engine: State<'_, AudioEngineState>,
) -> () {
    let _ = playback_control::previous(&domain, &engine);
}
