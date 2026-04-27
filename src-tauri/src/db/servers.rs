//! Stream server configuration database operations

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Database stream server record
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStreamServer {
    pub id: String,
    pub server_type: String,
    pub server_name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub legacy_auth: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music_folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    pub enabled: bool,
    pub created_at: i64,
}

/// Input data for saving a stream server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamServerInput {
    pub server_type: String,
    pub server_name: String,
    pub server_url: String,
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_auth: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Scan configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    pub id: Option<i64>,
    pub directories: Vec<String>,
    pub skip_short: bool,
    pub min_duration: f64,
    pub last_scan_at: Option<i64>,
}

/// Generate a server ID from URL and username
fn generate_server_id(server_url: &str, username: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(server_url.as_bytes());
    hasher.update(username.as_bytes());
    let result = hasher.finalize();
    format!("server-{:x}", result)[..32].to_string()
}

fn normalize_server_type(server_type: &str) -> String {
    if server_type.eq_ignore_ascii_case("opensubsonic") {
        "subsonic".to_string()
    } else {
        server_type.to_string()
    }
}

/// Save or update a stream server configuration
/// Returns the server ID
pub fn save_stream_server(conn: &Connection, input: &StreamServerInput) -> Result<String> {
    let id = generate_server_id(&input.server_url, &input.username);
    let normalized_server_type = normalize_server_type(&input.server_type);

    conn.execute(
        "INSERT OR REPLACE INTO stream_servers
         (id, server_type, server_name, server_url, username, password,
          access_token, user_id, legacy_auth, music_folder_id, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 COALESCE(?9, (SELECT legacy_auth FROM stream_servers WHERE id = ?1), 0),
                 COALESCE(?10, (SELECT music_folder_id FROM stream_servers WHERE id = ?1), NULL),
                 COALESCE(?11, (SELECT enabled FROM stream_servers WHERE id = ?1), 1),
                 COALESCE((SELECT created_at FROM stream_servers WHERE id = ?1), strftime('%s','now')))",
        params![
            id,
            normalized_server_type,
            input.server_name,
            input.server_url,
            input.username,
            input.password,
            input.access_token,
            input.user_id,
            input.legacy_auth.map(|legacy_auth| if legacy_auth { 1 } else { 0 }),
            input.music_folder_id,
            input.enabled.map(|enabled| if enabled { 1 } else { 0 }),
        ],
    )?;

    Ok(id)
}

/// Get all stream servers
pub fn get_stream_servers(conn: &Connection) -> Result<Vec<DbStreamServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, server_type, server_name, server_url, username, password,
                access_token, user_id, legacy_auth, music_folder_id, enabled, created_at
         FROM stream_servers
         ORDER BY created_at",
    )?;

    let servers = stmt
        .query_map([], |row| {
            Ok(DbStreamServer {
                id: row.get(0)?,
                server_type: row.get(1)?,
                server_name: row.get(2)?,
                server_url: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                access_token: row.get(6)?,
                user_id: row.get(7)?,
                legacy_auth: row.get::<_, i32>(8)? != 0,
                music_folder_id: row.get(9)?,
                enabled: row.get::<_, i32>(10)? != 0,
                created_at: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(servers)
}

/// Get a single stream server configuration by id.
pub fn get_stream_server_by_id(conn: &Connection, server_id: &str) -> Result<Option<DbStreamServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, server_type, server_name, server_url, username, password,
                access_token, user_id, legacy_auth, music_folder_id, enabled, created_at
         FROM stream_servers
         WHERE id = ?1
         LIMIT 1",
    )?;

    let row = stmt.query_row(params![server_id], |row| {
        Ok(DbStreamServer {
            id: row.get(0)?,
            server_type: row.get(1)?,
            server_name: row.get(2)?,
            server_url: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            access_token: row.get(6)?,
            user_id: row.get(7)?,
            legacy_auth: row.get::<_, i32>(8)? != 0,
            music_folder_id: row.get(9)?,
            enabled: row.get::<_, i32>(10)? != 0,
            created_at: row.get(11)?,
        })
    });

    match row {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Delete a stream server and all associated songs
pub fn delete_stream_server(conn: &Connection, server_id: &str) -> Result<()> {
    // Delete associated songs first
    conn.execute("DELETE FROM songs WHERE server_id = ?1", [server_id])?;

    // Delete the server config
    conn.execute("DELETE FROM stream_servers WHERE id = ?1", [server_id])?;

    Ok(())
}

/// Delete all stream servers
pub fn clear_stream_servers(conn: &Connection) -> Result<()> {
    // Delete all stream songs
    conn.execute("DELETE FROM songs WHERE source_type = 'stream'", [])?;
    // Delete all server configs
    conn.execute("DELETE FROM stream_servers", [])?;
    Ok(())
}

/// Save scan configuration
pub fn save_scan_config(conn: &Connection, config: &ScanConfig) -> Result<()> {
    let directories_json =
        serde_json::to_string(&config.directories).unwrap_or_else(|_| "[]".to_string());

    // We keep only one scan config, so delete and insert
    conn.execute("DELETE FROM scan_configs", [])?;
    conn.execute(
        "INSERT INTO scan_configs (directories, skip_short, min_duration, last_scan_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            directories_json,
            if config.skip_short { 1 } else { 0 },
            config.min_duration,
            config.last_scan_at,
        ],
    )?;

    Ok(())
}

/// Get scan configuration
pub fn get_scan_config(conn: &Connection) -> Result<Option<ScanConfig>> {
    let mut stmt = conn.prepare(
        "SELECT id, directories, skip_short, min_duration, last_scan_at
         FROM scan_configs
         LIMIT 1",
    )?;

    let config = stmt.query_row([], |row| {
        let id: i64 = row.get(0)?;
        let directories_json: String = row.get(1)?;
        let skip_short: i32 = row.get(2)?;
        let min_duration: f64 = row.get(3)?;
        let last_scan_at: Option<i64> = row.get(4)?;

        let directories: Vec<String> = serde_json::from_str(&directories_json).unwrap_or_default();

        Ok(ScanConfig {
            id: Some(id),
            directories,
            skip_short: skip_short != 0,
            min_duration,
            last_scan_at,
        })
    });

    match config {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Clear scan configuration
pub fn clear_scan_config(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM scan_configs", [])?;
    Ok(())
}
