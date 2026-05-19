//! Linux implementation using MPRIS2 (org.mpris.MediaPlayer2) over D-Bus.
//!
//! Uses `zbus 5` with the blocking API so we don't need to introduce a tokio
//! runtime into the sync `MediaController` interface. zbus runs its own
//! internal executor on a dedicated thread, so calls from any thread (including
//! Tauri's command pool) are safe.
//!
//! Property change notifications go out via the standard
//! `org.freedesktop.DBus.Properties.PropertiesChanged` signal, emitted manually
//! through `Connection::emit_signal`. The `Seeked` signal on the Player
//! interface is emitted the same way.

use super::MediaController;
use crate::models::*;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::sync::{Arc, Mutex, OnceLock};

use zbus::blocking::{connection, Connection};
use zbus::interface;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

const BUS_NAME: &str = "org.mpris.MediaPlayer2.bayin";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";
const TRACK_ID: &str = "/org/mpris/bayin/track/current";

// ── Event sink ─────────────────────────────────────────────────────
type EventSink = Arc<Mutex<Option<Box<dyn Fn(MediaControlEvent) + Send>>>>;
static EVENT_SINK: OnceLock<EventSink> = OnceLock::new();

fn event_sink() -> &'static EventSink {
    EVENT_SINK.get_or_init(|| Arc::new(Mutex::new(None)))
}

pub fn set_event_handler(handler: Box<dyn Fn(MediaControlEvent) + Send>) {
    let sink = event_sink();
    if let Ok(mut guard) = sink.lock() {
        *guard = Some(handler);
    }
}

fn emit(event_type: MediaControlEventType) {
    let sink = event_sink();
    if let Ok(guard) = sink.lock() {
        if let Some(handler) = guard.as_ref() {
            handler(MediaControlEvent { event_type });
        }
    }
}

// ── Shared state ───────────────────────────────────────────────────
#[derive(Default, Clone)]
struct PlayerState {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_us: i64,
    artwork_url: Option<String>,
    /// "Playing" | "Paused" | "Stopped" — the literal MPRIS enum string.
    playback_status: String,
    position_us: i64,
}

fn build_metadata(state: &PlayerState) -> HashMap<String, OwnedValue> {
    let mut map = HashMap::new();
    // OwnedValue only has `From` for primitives; complex types (String, Vec<String>,
    // ObjectPath) must round-trip through `Value` first.
    fn owned<'a, T: Into<Value<'a>>>(t: T) -> Option<OwnedValue> {
        OwnedValue::try_from(t.into()).ok()
    }

    if let Ok(path) = ObjectPath::try_from(TRACK_ID) {
        if let Some(v) = owned(path) {
            map.insert("mpris:trackid".to_string(), v);
        }
    }
    if state.duration_us > 0 {
        if let Ok(v) = OwnedValue::try_from(state.duration_us) {
            map.insert("mpris:length".to_string(), v);
        }
    }
    if let Some(ref url) = state.artwork_url {
        if let Some(v) = owned(url.clone()) {
            map.insert("mpris:artUrl".to_string(), v);
        }
    }
    if !state.title.is_empty() {
        if let Some(v) = owned(state.title.clone()) {
            map.insert("xesam:title".to_string(), v);
        }
    }
    if let Some(ref artist) = state.artist {
        if let Some(v) = owned(vec![artist.clone()]) {
            map.insert("xesam:artist".to_string(), v);
        }
    }
    if let Some(ref album) = state.album {
        if let Some(v) = owned(album.clone()) {
            map.insert("xesam:album".to_string(), v);
        }
    }
    map
}

// ── MPRIS root interface (org.mpris.MediaPlayer2) ──────────────────
struct RootIface;

#[interface(name = "org.mpris.MediaPlayer2")]
impl RootIface {
    fn raise(&self) {}
    fn quit(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool { false }
    #[zbus(property)]
    fn can_raise(&self) -> bool { false }
    #[zbus(property)]
    fn has_track_list(&self) -> bool { false }
    #[zbus(property)]
    fn identity(&self) -> String { "BaYin".to_string() }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> { Vec::new() }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> { Vec::new() }
}

// ── MPRIS Player interface (org.mpris.MediaPlayer2.Player) ─────────
struct PlayerIface {
    state: Arc<Mutex<PlayerState>>,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerIface {
    fn next(&self) { emit(MediaControlEventType::Next); }
    fn previous(&self) { emit(MediaControlEventType::Previous); }
    fn pause(&self) { emit(MediaControlEventType::Pause); }
    fn play_pause(&self) { emit(MediaControlEventType::PlayPause); }
    fn stop(&self) { emit(MediaControlEventType::Stop); }
    fn play(&self) { emit(MediaControlEventType::Play); }

    /// Seek relative to the current position (offset is in microseconds).
    fn seek(&self, offset_us: i64) {
        let cur = self.state.lock().map(|s| s.position_us).unwrap_or(0);
        let new_us = (cur + offset_us).max(0);
        emit(MediaControlEventType::SeekTo(new_us as f64 / 1_000_000.0));
    }

    /// Seek to an absolute position (position is in microseconds).
    fn set_position(&self, _track_id: ObjectPath<'_>, position_us: i64) {
        emit(MediaControlEventType::SeekTo(position_us.max(0) as f64 / 1_000_000.0));
    }

    fn open_uri(&self, _uri: String) {}

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.state.lock().map(|s| s.playback_status.clone()).unwrap_or_else(|_| "Stopped".into())
    }

    #[zbus(property)]
    fn loop_status(&self) -> String { "None".to_string() }
    #[zbus(property)]
    fn set_loop_status(&self, _v: String) {}

    #[zbus(property)]
    fn rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn set_rate(&self, _v: f64) {}

    #[zbus(property)]
    fn shuffle(&self) -> bool { false }
    #[zbus(property)]
    fn set_shuffle(&self, _v: bool) {}

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        self.state.lock().map(|s| build_metadata(&s)).unwrap_or_default()
    }

    #[zbus(property)]
    fn volume(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn set_volume(&self, _v: f64) {}

    #[zbus(property)]
    fn position(&self) -> i64 {
        self.state.lock().map(|s| s.position_us).unwrap_or(0)
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 { 1.0 }

    #[zbus(property)]
    fn can_go_next(&self) -> bool { true }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool { true }
    #[zbus(property)]
    fn can_play(&self) -> bool { true }
    #[zbus(property)]
    fn can_pause(&self) -> bool { true }
    #[zbus(property)]
    fn can_seek(&self) -> bool { true }
    #[zbus(property)]
    fn can_control(&self) -> bool { true }
}

// ── Controller ─────────────────────────────────────────────────────
pub struct LinuxController {
    state: Arc<Mutex<PlayerState>>,
    connection: Option<Connection>,
    initialized: bool,
}

impl LinuxController {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PlayerState {
                playback_status: "Stopped".to_string(),
                ..Default::default()
            })),
            connection: None,
            initialized: false,
        }
    }

    /// Emit `org.freedesktop.DBus.Properties.PropertiesChanged` for the
    /// Player interface with a single changed property.
    fn emit_player_property_changed(&self, prop: &str, value: Value<'_>) {
        let conn = match &self.connection {
            Some(c) => c,
            None => return,
        };
        let mut changed: HashMap<&str, Value<'_>> = HashMap::new();
        changed.insert(prop, value);
        let invalidated: Vec<&str> = Vec::new();
        let body = (PLAYER_IFACE, changed, invalidated);
        if let Err(e) = conn.emit_signal(
            None::<&str>,
            OBJECT_PATH,
            PROPS_IFACE,
            "PropertiesChanged",
            &body,
        ) {
            log::warn!("[system-media] PropertiesChanged emit failed: {}", e);
        }
    }

    fn emit_seeked(&self, position_us: i64) {
        let conn = match &self.connection {
            Some(c) => c,
            None => return,
        };
        if let Err(e) = conn.emit_signal(
            None::<&str>,
            OBJECT_PATH,
            PLAYER_IFACE,
            "Seeked",
            &position_us,
        ) {
            log::warn!("[system-media] Seeked emit failed: {}", e);
        }
    }
}

fn box_err<E: StdError + Send + 'static>(e: E) -> Box<dyn StdError + Send> {
    Box::new(e)
}

fn poisoned() -> Box<dyn StdError + Send> {
    Box::new(std::io::Error::other("LinuxController state mutex poisoned"))
}

impl MediaController for LinuxController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        if self.initialized {
            return Ok(());
        }
        let conn = connection::Builder::session()
            .map_err(box_err)?
            .name(BUS_NAME)
            .map_err(box_err)?
            .serve_at(OBJECT_PATH, RootIface)
            .map_err(box_err)?
            .serve_at(OBJECT_PATH, PlayerIface { state: self.state.clone() })
            .map_err(box_err)?
            .build()
            .map_err(box_err)?;
        self.connection = Some(conn);
        self.initialized = true;
        log::info!("[system-media] Linux MPRIS service registered as {}", BUS_NAME);
        Ok(())
    }

    fn set_metadata(&mut self, meta: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> {
        let metadata_value = {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            state.title = meta.title.clone();
            state.artist = meta.artist.clone();
            state.album = meta.album.clone();
            state.duration_us = meta
                .duration
                .map(|d| (d * 1_000_000.0) as i64)
                .unwrap_or(0);
            state.artwork_url = meta.artwork_url.clone();
            build_metadata(&state)
        };
        // OwnedValue → Value lifetime-extending dance: build a dict Value.
        let dict_value: Value<'_> = Value::from(metadata_value);
        self.emit_player_property_changed("Metadata", dict_value);
        Ok(())
    }

    fn set_playback_status(&mut self, status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> {
        let status_str = match status {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        };
        {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            state.playback_status = status_str.to_string();
        }
        self.emit_player_property_changed("PlaybackStatus", Value::from(status_str.to_string()));
        Ok(())
    }

    fn set_position(&mut self, position_secs: f64) -> Result<(), Box<dyn StdError + Send>> {
        let position_us = (position_secs * 1_000_000.0) as i64;
        {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            state.position_us = position_us;
        }
        // MPRIS spec: emit Seeked signal when the position changes discontinuously.
        // We don't try to distinguish "tick" vs "seek" here — callers decide
        // when to call set_position.
        self.emit_seeked(position_us);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        let metadata_value = {
            let mut state = self.state.lock().map_err(|_| poisoned())?;
            *state = PlayerState {
                playback_status: "Stopped".to_string(),
                ..Default::default()
            };
            build_metadata(&state)
        };
        self.emit_player_property_changed("Metadata", Value::from(metadata_value));
        self.emit_player_property_changed(
            "PlaybackStatus",
            Value::from("Stopped".to_string()),
        );
        Ok(())
    }
}
