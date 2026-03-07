use bayin_playback::PlayMode;
use rand::Rng;

use crate::audio_engine::{engine::AudioCommand, AudioEngineState};
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

pub(crate) fn play_index(
    index: usize,
    domain: &PlaybackDomainState,
    engine: &AudioEngineState,
) -> bool {
    let file = {
        let mut d = domain.0.lock().unwrap();
        if d.queue.is_empty() || index >= d.queue.len() {
            return false;
        }
        d.index = index;
        let resolved = resolve_source(&d.queue[index].file_path);
        d.queue[index].file_path = resolved.clone();
        resolved
    };

    let engine = engine.lock().unwrap();
    engine.send(AudioCommand::Play { source: file });
    true
}

pub(crate) fn next(domain: &PlaybackDomainState, engine: &AudioEngineState) -> bool {
    let (cur, mode, len) = {
        let d = domain.0.lock().unwrap();
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

    play_index(idx, domain, engine)
}

pub(crate) fn previous(domain: &PlaybackDomainState, engine: &AudioEngineState) -> bool {
    let (cur, mode, len) = {
        let d = domain.0.lock().unwrap();
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

    play_index(idx, domain, engine)
}
