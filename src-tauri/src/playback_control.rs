use bayin_playback::PlayMode;
use rand::Rng;
use std::sync::{Mutex, OnceLock};
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
    server_id: String,
}

fn resolve_source(app_handle: &AppHandle, file_path: &str) -> String {
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

    let db = match app_handle.try_state::<DbState>() {
        Some(state) => state,
        None => return file_path.to_string(),
    };
    let config = {
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(poisoned) => poisoned.into_inner(),
        };
        match crate::db::servers::get_stream_server_by_id(&conn, &parsed.server_id) {
            Ok(Some(server)) => crate::models::StreamServerConfig::from(&server),
            Ok(None) => return file_path.to_string(),
            Err(err) => {
                log::warn!("Failed to load stream server {}: {}", parsed.server_id, err);
                return file_path.to_string();
            }
        }
    };

    if config.is_subsonic() {
        crate::utils::subsonic::get_stream_url(&config, &parsed.song_id)
    } else if config.is_jellyfin_like() {
        crate::utils::jellyfin::get_stream_url(&config, &parsed.song_id)
    } else {
        file_path.to_string()
    }
}

static LAST_RECORDED_PLAY: OnceLock<Mutex<Option<(String, i64)>>> = OnceLock::new();

fn last_recorded_play() -> &'static Mutex<Option<(String, i64)>> {
    LAST_RECORDED_PLAY.get_or_init(|| Mutex::new(None))
}

fn record_play(app_handle: &AppHandle, track_id: &str) {
    const CONSECUTIVE_PLAY_DEBOUNCE_SECS: i64 = 15;

    if track_id.is_empty() {
        return;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    {
        let mut last = match last_recorded_play().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some((last_track_id, last_at)) = last.as_ref() {
            if last_track_id == track_id && now.saturating_sub(*last_at) < CONSECUTIVE_PLAY_DEBOUNCE_SECS {
                return;
            }
        }

        *last = Some((track_id.to_string(), now));
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
        let resolved = resolve_source(app_handle, &d.queue[index].file_path);
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
