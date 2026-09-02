//! Database module for SQLite persistence
//!
//! This module provides persistent storage for songs, albums, artists,
//! stream server configurations, and scan settings.

pub mod init;
pub mod songs;
pub mod albums;
pub mod servers;
pub mod playlists;
pub mod resume;
pub mod cache;
pub mod creds_migrate;

use rusqlite::Connection;
use std::sync::Mutex;

pub use init::*;
pub use songs::*;
pub use albums::*;
pub use servers::*;
pub use resume::*;
pub use cache::*;

/// Database state wrapper for Tauri managed state
pub struct DbState(pub Mutex<Connection>);
