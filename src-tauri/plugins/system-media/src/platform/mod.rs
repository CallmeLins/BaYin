use crate::models::*;
use std::error::Error as StdError;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
#[allow(deprecated)] // cocoa crate is deprecated but still functional
pub mod macos;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "android")]
pub mod android;

/// Trait that each platform must implement.
pub trait MediaController: Send {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>>;
    fn set_metadata(&mut self, metadata: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>>;
    fn set_playback_status(&mut self, status: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>>;
    fn set_position(&mut self, position_secs: f64) -> Result<(), Box<dyn StdError + Send>>;
    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>>;
}

/// Create the platform-specific controller.
pub fn create_controller() -> Box<dyn MediaController> {
    #[cfg(target_os = "windows")]
    { Box::new(windows::WindowsController::new()) }
    #[cfg(target_os = "macos")]
    { Box::new(macos::MacOsController::new()) }
    #[cfg(target_os = "linux")]
    { Box::new(linux::LinuxController::new()) }
    #[cfg(target_os = "android")]
    { Box::new(android::AndroidController::new()) }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
    { Box::new(StubController) }
}

/// Stub for platforms without media control support.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
struct StubController;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
impl MediaController for StubController {
    fn initialize(&mut self) -> Result<(), Box<dyn StdError + Send>> { Ok(()) }
    fn set_metadata(&mut self, _: &MediaMetadata) -> Result<(), Box<dyn StdError + Send>> { Ok(()) }
    fn set_playback_status(&mut self, _: PlaybackStatus) -> Result<(), Box<dyn StdError + Send>> { Ok(()) }
    fn set_position(&mut self, _: f64) -> Result<(), Box<dyn StdError + Send>> { Ok(()) }
    fn clear(&mut self) -> Result<(), Box<dyn StdError + Send>> { Ok(()) }
}
