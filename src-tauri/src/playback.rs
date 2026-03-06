use std::sync::Mutex;

use bayin_playback::{PlayMode, Track};

/// Shared playback domain state (queue + mode).
///
/// Audio decoding/output remains in `audio_engine`; this module is the cross-platform
/// source of truth for "what should be playing".
#[derive(Debug, Default, Clone)]
pub struct PlaybackDomain {
    pub queue: Vec<Track>,
    pub index: usize,
    pub mode: PlayMode,
}

impl PlaybackDomain {
    // Reserved for a future "get state" command if we decide to expose this directly to the UI.
}

pub struct PlaybackDomainState(pub Mutex<PlaybackDomain>);

impl PlaybackDomainState {
    pub fn new() -> Self {
        Self(Mutex::new(PlaybackDomain::default()))
    }
}
