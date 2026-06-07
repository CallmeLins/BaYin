use bayin_playback::PlayMode;
use rand::Rng;
use tauri::{AppHandle, Manager};

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
use crate::db::DbState;
use crate::playback::PlaybackDomainState;

fn random_other_index(len: usize, current: usize) -> usize {
    if len <= 1 {
        return current;
    }

    let mut rng = rand::thread_rng();
    loop {
        let idx = rng.gen_range(0..len);
        if idx != current {
            return idx;
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamFilePath {
    #[serde(rename = "type")]
    kind: String,
    song_id: String,
    config: crate::models::StreamServerConfig,
}

fn resolve_source(file_path: &str) -> String {
    // The frontend sometimes stores a JSON "virtual path" for stream items.
    // Resolve it to a real playable URL so that native/system controls can work too.
    if !file_path.trim_start().starts_with('{') {
        return file_path.to_string();
    }

    let parsed: Result<StreamFilePath, _> = serde_json::from_str(file_path);
    let parsed = match parsed {
        Ok(p) if p.kind == "stream" => p,
        _ => return file_path.to_string(),
    };

    if parsed.config.is_subsonic() {
        crate::utils::subsonic::get_stream_url(&parsed.config, &parsed.song_id)
    } else if parsed.config.is_jellyfin_like() {
        crate::utils::jellyfin::get_stream_url(&parsed.config, &parsed.song_id)
    } else {
        file_path.to_string()
    }
}

fn record_play(app_handle: &AppHandle, track_id: &str) {
    if track_id.is_empty() {
        return;
    }

    let db = match app_handle.try_state::<DbState>() {
        Some(state) => state,
        None => return,
    };

    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Err(err) = crate::db::songs::record_play(&conn, track_id) {
        log::warn!("Failed to record play for track {}: {}", track_id, err);
    }
}

pub(crate) fn play_index(
    index: usize,
    domain: &PlaybackDomainState,
    engine: &AudioEngineState,
    app_handle: &AppHandle,
) -> bool {
    let (file, track_id) = {
        // 使用 unwrap_or_else 但避免格式化字符串的开销
        let mut d = match domain.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if d.queue.is_empty() || index >= d.queue.len() {
            return false;
        }
        d.index = index;
        let resolved = resolve_source(&d.queue[index].file_path);
        d.queue[index].file_path = resolved.clone();
        (resolved, d.queue[index].id.clone())
    };

    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Play { source: file });
    record_play(app_handle, &track_id);
    true
}

pub(crate) fn next(domain: &PlaybackDomainState, engine: &AudioEngineState, app_handle: &AppHandle) -> bool {
    let (cur, mode, len) = {
        let d = match domain.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if d.queue.is_empty() {
            return false;
        }
        (d.index, d.mode, d.queue.len())
    };

    let idx = match mode {
        PlayMode::RepeatOne => cur,
        PlayMode::Shuffle => random_other_index(len, cur),
        _ => (cur + 1) % len,
    };

    play_index(idx, domain, engine, app_handle)
}

pub(crate) fn previous(domain: &PlaybackDomainState, engine: &AudioEngineState, app_handle: &AppHandle) -> bool {
    let (cur, mode, len) = {
        let d = match domain.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if d.queue.is_empty() {
            return false;
        }
        (d.index, d.mode, d.queue.len())
    };

    let idx = match mode {
        PlayMode::RepeatOne => cur,
        PlayMode::Shuffle => random_other_index(len, cur),
        _ => (cur + len - 1) % len,
    };

    play_index(idx, domain, engine, app_handle)
}
