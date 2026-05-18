//! Linux implementation using MPRIS (D-Bus).
//! TODO: Implement MPRIS2 interface via zbus or dbus crate.

use super::MediaController;
use crate::models::*;
use std::error::Error as StdError;

pub struct LinuxController {
    initialized: bool,
}

impl LinuxController {
    pub fn new() -> Self {
        LinuxController { initialized: false }
    }
}

impl MediaController for LinuxController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        // TODO: Register MPRIS2 D-Bus service
        self.initialized = true;
        Ok(())
    }

    fn set_metadata(&mut self, _meta: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> {
        // TODO: Set MPRIS metadata
        Ok(())
    }

    fn set_playback_status(&mut self, _status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> {
        // TODO: Set MPRIS playback status
        Ok(())
    }

    fn set_position(&mut self, _position_secs: f64) -> Result<(), Box<dyn StdError + Send>> {
        // TODO: Set MPRIS position
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> {
        // TODO: Clear MPRIS metadata
        Ok(())
    }
}
