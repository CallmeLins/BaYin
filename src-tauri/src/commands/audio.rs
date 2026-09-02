use crate::audio_engine::engine::{AudioCommand, PlaybackState};
use crate::audio_engine::AudioEngineState;
use tauri::State;

#[tauri::command]
pub fn audio_play(
    source: String,
    engine: State<'_, AudioEngineState>,
    headers: Option<Vec<(String, String)>>,
) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Play {
        source,
        headers,
        cache: None, // audio_play 命令层无 server/song 身份；断点续播经 playback_control 走 cache
    });
}

#[tauri::command]
pub fn audio_pause(engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Pause);
}

#[tauri::command]
pub fn audio_resume(engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Resume);
}

#[tauri::command]
pub fn audio_stop(engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Stop);
}

#[tauri::command]
pub fn audio_seek(position_secs: f64, engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Seek { position_secs });
}

#[tauri::command]
pub fn audio_set_volume(volume: f32, engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::SetVolume { volume });
}

#[tauri::command]
pub fn audio_set_eq_bands(gains: Vec<f32>, engine: State<'_, AudioEngineState>) {
    if gains.len() != 10 {
        return;
    }
    let mut arr = [0.0f32; 10];
    arr.copy_from_slice(&gains);
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::SetEqBands { gains: arr });
}

#[tauri::command]
pub fn audio_set_eq_enabled(enabled: bool, engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::SetEqEnabled { enabled });
}

#[tauri::command]
pub fn audio_enable_visualization(enabled: bool, engine: State<'_, AudioEngineState>) {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::EnableVisualization { enabled });
}

#[tauri::command]
pub fn audio_get_state(engine: State<'_, AudioEngineState>) -> PlaybackState {
    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.state.lock()
        .map(|state| state.clone())
        .unwrap_or_default()
}
