use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayMode {
    Sequence,
    RepeatOne,
    Shuffle,
}

impl Default for PlayMode {
    fn default() -> Self {
        Self::Sequence
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub file_path: String,
    pub artwork_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueState {
    pub tracks: Vec<Track>,
    pub index: usize,
    pub mode: PlayMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PositionState {
    pub is_playing: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowPlayingState {
    pub queue: QueueState,
    pub position: PositionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaybackCommand {
    PlayIndex { index: usize },
    TogglePlay,
    Play,
    Pause,
    Next,
    Previous,
    Seek { position_secs: f64 },
    SetPlayMode { mode: PlayMode },
    SetQueue { tracks: Vec<Track>, index: usize, mode: PlayMode },
}

pub trait PlaybackControl: Send + Sync + 'static {
    fn handle(&self, cmd: PlaybackCommand);
}
