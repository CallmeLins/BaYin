use serde::{Deserialize, Serialize};

/// Playback status for the system media session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

/// Metadata for the currently playing track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub artwork_url: Option<String>,
}

/// Event emitted by the system media controls (lock screen, media keys, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaControlEvent {
    pub event_type: MediaControlEventType,
}

/// Type of media control event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaControlEventType {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    SeekTo(f64),
}
