//! Database initialization and migration

use rusqlite::{Connection, Result};
use std::path::Path;

const CURRENT_SCHEMA_VERSION: i32 = 5;

/// Initialize the database with tables and indexes
pub fn init_db(conn: &Connection) -> Result<()> {
    // Create schema_version table first
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY
        )",
        [],
    )?;

    // Check current version
    let current_version: i32 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    if current_version < CURRENT_SCHEMA_VERSION {
        run_migrations(conn, current_version)?;
    }

    Ok(())
}

/// Run database migrations from current version to latest
fn run_migrations(conn: &Connection, from_version: i32) -> Result<()> {
    if from_version < 1 {
        migrate_v1(conn)?;
    }
    if from_version < 2 {
        migrate_v2(conn)?;
    }
    if from_version < 3 {
        migrate_v3(conn)?;
    }
    if from_version < 4 {
        migrate_v4(conn)?;
    }
    if from_version < 5 {
        migrate_v5(conn)?;
    }

    Ok(())
}

/// Version 1: Initial schema
fn migrate_v1(conn: &Connection) -> Result<()> {
    // Songs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS songs (
            id              TEXT PRIMARY KEY,
            title           TEXT NOT NULL,
            artist          TEXT NOT NULL DEFAULT '未知艺术家',
            album           TEXT NOT NULL DEFAULT '未知专辑',
            duration        REAL NOT NULL DEFAULT 0.0,
            file_path       TEXT NOT NULL,
            file_size       INTEGER NOT NULL DEFAULT 0,
            is_hr           INTEGER DEFAULT 0,
            is_sq           INTEGER DEFAULT 0,
            cover_url       TEXT,
            source_type     TEXT NOT NULL DEFAULT 'local',
            server_id       TEXT,
            server_song_id  TEXT,
            stream_info     TEXT,
            file_modified   INTEGER,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            updated_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        )",
        [],
    )?;

    // Stream servers table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS stream_servers (
            id              TEXT PRIMARY KEY,
            server_type     TEXT NOT NULL,
            server_name     TEXT NOT NULL,
            server_url      TEXT NOT NULL,
            username        TEXT NOT NULL,
            password        TEXT NOT NULL,
            access_token    TEXT,
            user_id         TEXT,
            enabled         INTEGER NOT NULL DEFAULT 1,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        )",
        [],
    )?;

    // Scan configs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS scan_configs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            directories     TEXT NOT NULL,
            skip_short      INTEGER DEFAULT 1,
            min_duration    REAL DEFAULT 60.0,
            last_scan_at    INTEGER
        )",
        [],
    )?;

    // Create indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_songs_source ON songs(source_type)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_songs_server ON songs(server_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_songs_album ON songs(album)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_songs_artist ON songs(artist)",
        [],
    )?;

    // Record version
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [1])?;

    Ok(())
}

/// Version 2: Add cover_hash column for cached covers
fn migrate_v2(conn: &Connection) -> Result<()> {
    // Add cover_hash column to songs table
    conn.execute(
        "ALTER TABLE songs ADD COLUMN cover_hash TEXT",
        [],
    )?;

    // Create cover_cache table for tracking cached covers
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cover_cache (
            hash            TEXT PRIMARY KEY,
            mid_path        TEXT,
            original_path   TEXT,
            file_size       INTEGER DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        )",
        [],
    )?;

    // Create index for cover_hash lookups
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_songs_cover_hash ON songs(cover_hash)",
        [],
    )?;

    // Record version
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [2])?;

    Ok(())
}

/// Version 3: Add audio format columns (format, bit_depth, sample_rate, bitrate, channels)
fn migrate_v3(conn: &Connection) -> Result<()> {
    conn.execute("ALTER TABLE songs ADD COLUMN format TEXT", [])?;
    conn.execute("ALTER TABLE songs ADD COLUMN bit_depth INTEGER", [])?;
    conn.execute("ALTER TABLE songs ADD COLUMN sample_rate INTEGER", [])?;
    conn.execute("ALTER TABLE songs ADD COLUMN bitrate INTEGER", [])?;
    conn.execute("ALTER TABLE songs ADD COLUMN channels INTEGER", [])?;

    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [3])?;

    Ok(())
}

/// Version 4: Stream playlists cache (remote playlist index + items).
fn migrate_v4(conn: &Connection) -> Result<()> {
    // Playlist summaries fetched from remote servers.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS stream_playlists (
            server_id       TEXT NOT NULL,
            playlist_id     TEXT NOT NULL,
            name            TEXT NOT NULL,
            song_count      INTEGER NOT NULL DEFAULT 0,
            kind            TEXT NOT NULL DEFAULT 'manual',
            updated_at      INTEGER,
            synced_at       INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            PRIMARY KEY (server_id, playlist_id)
        )",
        [],
    )?;

    // Playlist items (song ids) fetched from remote servers.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS stream_playlist_items (
            server_id       TEXT NOT NULL,
            playlist_id     TEXT NOT NULL,
            position        INTEGER NOT NULL,
            song_id         TEXT NOT NULL,
            synced_at       INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            PRIMARY KEY (server_id, playlist_id, position)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_stream_playlists_server ON stream_playlists(server_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_stream_playlist_items_server_playlist ON stream_playlist_items(server_id, playlist_id)",
        [],
    )?;

    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [4])?;
    Ok(())
}

/// Version 5: Add legacy auth toggle for Subsonic-compatible servers.
fn migrate_v5(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(stream_servers)")?;
    let has_legacy_auth = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|row| row.ok())
        .any(|name| name == "legacy_auth");

    if !has_legacy_auth {
        conn.execute(
            "ALTER TABLE stream_servers ADD COLUMN legacy_auth INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [5])?;
    Ok(())
}

/// Open or create a database at the given path
pub fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Enable foreign keys and WAL mode for better performance
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -64000;"
    )?;

    init_db(&conn)?;

    Ok(conn)
}
