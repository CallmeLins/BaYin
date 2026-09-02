use bayin_playback::PlayMode;
use rand::Rng;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
use crate::db::DbState;
use crate::playback::PlaybackDomainState;
use crate::utils::webdav;

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

/// 解析后的播放源：URL + 可选的认证请求头
struct ResolvedSource {
    url: String,
    headers: Option<Vec<(String, String)>>,
}

fn resolve_source(app_handle: &AppHandle, file_path: &str) -> ResolvedSource {
    // The frontend sometimes stores a JSON "virtual path" for stream items.
    // Resolve it to a real playable URL so that native/system controls can work too.
    if !file_path.trim_start().starts_with('{') {
        return ResolvedSource {
            url: file_path.to_string(),
            headers: None,
        };
    }

    let parsed: Result<StreamFilePath, _> = serde_json::from_str(file_path);
    let parsed = match parsed {
        Ok(p) if p.kind == "stream" => p,
        _ => {
            return ResolvedSource {
                url: file_path.to_string(),
                headers: None,
            }
        }
    };

    let db = match app_handle.try_state::<DbState>() {
        Some(state) => state,
        None => {
            return ResolvedSource {
                url: file_path.to_string(),
                headers: None,
            }
        }
    };
    let config = {
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(poisoned) => poisoned.into_inner(),
        };
        // A6：经凭据后端解析（含明文回退），防止迁移后断点续播拿空密码。
        match crate::db::servers::load_resolved_stream_config(&conn, db.credential_store(), &parsed.server_id) {
            Ok(Some(config)) => config,
            _ => {
                return ResolvedSource {
                    url: file_path.to_string(),
                    headers: None,
                }
            }
        }
    };

    if config.is_subsonic() {
        ResolvedSource {
            url: crate::utils::subsonic::get_stream_url(&config, &parsed.song_id),
            headers: None,
        }
    } else if config.is_jellyfin_like() {
        ResolvedSource {
            url: crate::utils::jellyfin::get_stream_url(&config, &parsed.song_id),
            headers: None,
        }
    } else if config.is_webdav() {
        ResolvedSource {
            url: webdav::stream_url(&config, &parsed.song_id),
            headers: Some(webdav::stream_headers(&config)),
        }
    } else {
        ResolvedSource {
            url: file_path.to_string(),
            headers: None,
        }
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
    let (resolved, track_id) = {
        // 使用 unwrap_or_else 但避免格式化字符串的开销
        let mut d = match domain.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if d.queue.is_empty() || index >= d.queue.len() {
            return false;
        }
        d.index = index;
        (
            resolve_source(app_handle, &d.queue[index].file_path),
            d.queue[index].id.clone(),
        )
    };

    let engine = match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    engine.send(AudioCommand::Play {
        source: resolved.url,
        headers: resolved.headers,
    });
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
