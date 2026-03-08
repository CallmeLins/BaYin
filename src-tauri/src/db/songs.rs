//! Song database operations

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database song record
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub file_path: String,
    pub file_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_sq: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_hash: Option<String>,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_song_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_modified: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
}

/// Input data for saving a song
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongInput {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub file_path: String,
    #[serde(default)]
    pub file_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_sq: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_song_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_modified: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
}

/// Get all songs from the database (fast loading, no cover data)
pub fn get_all_songs(conn: &Connection) -> Result<Vec<DbSong>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album, duration, file_path, file_size,
                is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
                stream_info, file_modified, format, bit_depth, sample_rate, bitrate, channels
         FROM songs
         ORDER BY title COLLATE NOCASE",
    )?;

    let songs = stmt
        .query_map([], |row| {
            Ok(DbSong {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                duration: row.get(4)?,
                file_path: row.get(5)?,
                file_size: row.get(6)?,
                is_hr: row.get::<_, Option<i32>>(7)?.map(|v| v != 0),
                is_sq: row.get::<_, Option<i32>>(8)?.map(|v| v != 0),
                cover_hash: row.get(9)?,
                source_type: row.get(10)?,
                server_id: row.get(11)?,
                server_song_id: row.get(12)?,
                stream_info: row.get(13)?,
                file_modified: row.get(14)?,
                format: row.get(15)?,
                bit_depth: row.get::<_, Option<u8>>(16)?,
                sample_rate: row.get::<_, Option<u32>>(17)?,
                bitrate: row.get::<_, Option<u32>>(18)?,
                channels: row.get::<_, Option<u8>>(19)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Save songs to database in batches (within a transaction)
pub fn save_songs(
    conn: &mut Connection,
    songs: &[SongInput],
    source_type: &str,
    server_id: Option<&str>,
) -> Result<usize> {
    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO songs
             (id, title, artist, album, duration, file_path, file_size,
              is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
              stream_info, file_modified, format, bit_depth, sample_rate, bitrate, channels, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, strftime('%s','now'))"
        )?;

        for song in songs {
            stmt.execute(params![
                song.id,
                song.title,
                song.artist,
                song.album,
                song.duration,
                song.file_path,
                song.file_size,
                song.is_hr.map(|v| if v { 1 } else { 0 }),
                song.is_sq.map(|v| if v { 1 } else { 0 }),
                song.cover_hash,
                source_type,
                server_id,
                song.server_song_id,
                song.stream_info,
                song.file_modified,
                song.format,
                song.bit_depth,
                song.sample_rate,
                song.bitrate,
                song.channels,
            ])?;
        }
    }

    tx.commit()?;
    Ok(songs.len())
}

/// Delete songs by source type (optionally filtered by server_id)
pub fn delete_songs_by_source(
    conn: &Connection,
    source_type: &str,
    server_id: Option<&str>,
) -> Result<usize> {
    let affected = if let Some(sid) = server_id {
        conn.execute(
            "DELETE FROM songs WHERE source_type = ?1 AND server_id = ?2",
            params![source_type, sid],
        )?
    } else {
        conn.execute(
            "DELETE FROM songs WHERE source_type = ?1",
            params![source_type],
        )?
    };

    Ok(affected)
}

/// Delete all songs
pub fn clear_all_songs(conn: &Connection) -> Result<usize> {
    let affected = conn.execute("DELETE FROM songs", [])?;
    Ok(affected)
}

/// Get count of songs
pub fn get_song_count(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM songs", [], |row| row.get(0))
}

/// Get count of songs by source
pub fn get_song_count_by_source(conn: &Connection, source_type: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM songs WHERE source_type = ?1",
        [source_type],
        |row| row.get(0),
    )
}

/// Get existing `cover_hash` values for songs of a given source/server.
///
/// This is used to preserve cached covers across re-scans (e.g. streaming libraries),
/// even if we delete-and-reinsert songs for that source.
pub fn get_cover_hashes_by_source(
    conn: &Connection,
    source_type: &str,
    server_id: Option<&str>,
) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    if let Some(sid) = server_id {
        let mut stmt = conn.prepare(
            "SELECT id, cover_hash FROM songs
             WHERE source_type = ?1 AND server_id = ?2 AND cover_hash IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![source_type, sid], |row| {
            let id: String = row.get(0)?;
            let hash: String = row.get(1)?;
            Ok((id, hash))
        })?;

        for row in rows {
            let (id, hash) = row?;
            map.insert(id, hash);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, cover_hash FROM songs
             WHERE source_type = ?1 AND cover_hash IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![source_type], |row| {
            let id: String = row.get(0)?;
            let hash: String = row.get(1)?;
            Ok((id, hash))
        })?;

        for row in rows {
            let (id, hash) = row?;
            map.insert(id, hash);
        }
    }

    Ok(map)
}

/// Update a song's cover hash (reference into CoverCache).
///
/// Currently used by Android system media integration for lazy cover caching.
#[cfg(target_os = "android")]
pub fn update_song_cover_hash(conn: &Connection, song_id: &str, cover_hash: &str) -> Result<usize> {
    conn.execute(
        "UPDATE songs SET cover_hash = ?1, updated_at = strftime('%s','now') WHERE id = ?2",
        params![cover_hash, song_id],
    )
}
