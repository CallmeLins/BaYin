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

use crate::source::creds::CredentialStore;

pub use init::*;
pub use songs::*;
pub use albums::*;
pub use servers::*;
pub use resume::*;
pub use cache::*;

/// Database state wrapper for Tauri managed state.
///
/// 第二个字段是「活动凭据后端」（keyring 或文件降级），在应用启动时用
/// `default_store(app_data_dir)` 构造。消费点构造 `StreamServerConfig` 时
/// 需经 `load_stream_config`（commands/streaming.rs）先从中解析 password /
/// access_token（含明文回退），否则迁移后连接会拿空密码。
pub struct DbState(pub Mutex<Connection>, pub Box<dyn CredentialStore>);

impl DbState {
    /// 访问活动凭据后端。
    pub fn credential_store(&self) -> &dyn CredentialStore {
        self.1.as_ref()
    }
}
