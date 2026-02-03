//! Song database operations

use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};

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
    pub cover_hash: Option<String>,  // SHA256 hash for cover lookup
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_song_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_modified: Option<i64>,
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
    pub cover_hash: Option<String>,  // SHA256 hash of cover image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_song_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_modified: Option<i64>,
}

/// Get all songs from the database (fast loading, no cover data)
pub fn get_all_songs(conn: &Connection) -> Result<Vec<DbSong>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album, duration, file_path, file_size,
                is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
                stream_info, file_modified
         FROM songs
         ORDER BY title COLLATE NOCASE"
    )?;

    let songs = stmt.query_map([], |row| {
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
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Get songs by source type
pub fn get_songs_by_source(conn: &Connection, source_type: &str) -> Result<Vec<DbSong>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album, duration, file_path, file_size,
                is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
                stream_info, file_modified
         FROM songs
         WHERE source_type = ?1
         ORDER BY title COLLATE NOCASE"
    )?;

    let songs = stmt.query_map([source_type], |row| {
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
        })
    })?.collect::<Result<Vec<_>>>()?;

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
              stream_info, file_modified, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, strftime('%s','now'))"
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
