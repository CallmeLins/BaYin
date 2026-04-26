use crate::audio_engine::{EqState, FftSnapshot, PlaybackState};
use crate::state::with_audio_engine;

pub fn audio_play(source: String) -> Result<(), String> {
    with_audio_engine(|engine| engine.play(source))
}

pub fn audio_pause() -> Result<(), String> {
    with_audio_engine(|engine| engine.pause())
}

pub fn audio_resume() -> Result<(), String> {
    with_audio_engine(|engine| engine.resume())
}

pub fn audio_stop() -> Result<(), String> {
    with_audio_engine(|engine| engine.stop())
}

pub fn audio_seek(position_secs: f64) -> Result<(), String> {
    with_audio_engine(|engine| engine.seek(position_secs))
}

pub fn audio_set_volume(volume: f32) -> Result<(), String> {
    with_audio_engine(|engine| engine.set_volume(volume))
}

pub fn audio_set_eq_enabled(enabled: bool) -> Result<(), String> {
    with_audio_engine(|engine| engine.set_eq_enabled(enabled))
}

pub fn audio_set_eq_gains(gains: [f32; 10]) -> Result<(), String> {
    with_audio_engine(|engine| engine.set_eq_gains(gains))
}

pub fn audio_get_state() -> Result<PlaybackState, String> {
    with_audio_engine(|engine| engine.playback_state())
}

pub fn audio_get_fft() -> Result<FftSnapshot, String> {
    with_audio_engine(|engine| engine.fft_snapshot())
}

pub fn audio_get_eq_state() -> Result<EqState, String> {
    with_audio_engine(|engine| engine.eq_state())
}
