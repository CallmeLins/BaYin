//! Song database operations

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SongUserStats {
    pub play_count: i64,
    pub last_played_at: Option<i64>,
    pub liked_at: Option<i64>,
}

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
    #[serde(default)]
    pub play_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

/// Helper function to map a row to DbSong
fn row_to_db_song(row: &rusqlite::Row) -> rusqlite::Result<DbSong> {
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
        play_count: row.get::<_, Option<i64>>(20)?.unwrap_or(0),
        last_played_at: row.get(21)?,
        liked_at: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
    })
}

const SELECT_COLUMNS: &str = "id, title, artist, album, duration, file_path, file_size,
        is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
        stream_info, file_modified, format, bit_depth, sample_rate, bitrate, channels,
        play_count, last_played_at, liked_at, created_at, updated_at";

/// Get all songs from the database (fast loading, no cover data)
pub fn get_all_songs(conn: &Connection) -> Result<Vec<DbSong>> {
    let sql = format!(
        "SELECT {} FROM songs ORDER BY title COLLATE NOCASE",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map([], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Get recently played songs (ordered by last_played_at DESC)
pub fn get_recently_played(conn: &Connection, limit: u32) -> Result<Vec<DbSong>> {
    let sql = format!(
        "SELECT {} FROM songs WHERE last_played_at IS NOT NULL ORDER BY last_played_at DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Get liked songs (ordered by liked_at DESC)
pub fn get_liked_songs(conn: &Connection, limit: u32) -> Result<Vec<DbSong>> {
    let sql = format!(
        "SELECT {} FROM songs WHERE liked_at IS NOT NULL ORDER BY liked_at DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Set or clear liked state for a song.
pub fn set_song_liked(conn: &Connection, song_id: &str, liked: bool) -> Result<Option<i64>> {
    let liked_at = if liked {
        Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        )
    } else {
        None
    };

    conn.execute(
        "UPDATE songs SET liked_at = ?1, updated_at = strftime('%s','now') WHERE id = ?2",
        params![liked_at, song_id],
    )?;

    Ok(liked_at)
}

/// Get most played songs (ordered by play_count DESC)
pub fn get_most_played(conn: &Connection, limit: u32) -> Result<Vec<DbSong>> {
    let sql = format!(
        "SELECT {} FROM songs WHERE play_count > 0 ORDER BY play_count DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Increment play count and update last_played_at for a song.
pub fn record_play(conn: &Connection, song_id: &str) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    conn.execute(
        "UPDATE songs SET play_count = play_count + 1, last_played_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, song_id],
    )?;

    Ok(())
}

/// Clear recently played history while preserving play_count statistics.
pub fn clear_recently_played(conn: &Connection) -> Result<usize> {
    conn.execute(
        "UPDATE songs SET last_played_at = NULL, updated_at = strftime('%s','now') WHERE last_played_at IS NOT NULL",
        [],
    )
}

/// Get top played artists (GROUP BY artist)
pub fn get_top_artists(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<(String, i64, i64, Option<String>, Option<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT artist, COUNT(*) as song_count, SUM(play_count) as total_plays, 
         (SELECT cover_hash FROM songs s2 WHERE s2.artist = songs.artist AND s2.cover_hash IS NOT NULL LIMIT 1) as cover_hash,
         (SELECT stream_info FROM songs s3 WHERE s3.artist = songs.artist AND s3.stream_info IS NOT NULL LIMIT 1) as stream_info
         FROM songs 
         GROUP BY artist 
         ORDER BY total_plays DESC, song_count DESC
         LIMIT ?1"
    )?;

    let artists = stmt
        .query_map(params![limit], |row| {
            let stream_info: Option<String> = row.get(4)?;
            // Extract coverUrl from stream_info JSON
            let stream_cover_url = stream_info.and_then(|info| {
                serde_json::from_str::<serde_json::Value>(&info)
                    .ok()
                    .and_then(|v| v.get("coverUrl")?.as_str().map(|s| s.to_string()))
            });
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                stream_cover_url,
            ))
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(artists)
}

/// Get newly added songs (recent N days, fallback to latest if not enough)
pub fn get_newly_added(conn: &Connection, days: i64, limit: u32) -> Result<Vec<DbSong>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff = now - days * 86400;

    // First try to get songs within the time range
    let sql = format!(
        "SELECT {} FROM songs WHERE created_at >= ?1 ORDER BY created_at DESC LIMIT ?2",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![cutoff, limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    // If we got enough songs, return them
    if songs.len() >= limit as usize {
        return Ok(songs);
    }

    // Otherwise, fallback to the latest songs regardless of time
    let sql = format!(
        "SELECT {} FROM songs ORDER BY created_at DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Get forgotten favorites (played before but not recently)
pub fn get_forgotten_favorites(conn: &Connection, days: i64, limit: u32) -> Result<Vec<DbSong>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cutoff = now - days * 86400;

    let sql = format!(
        "SELECT {} FROM songs WHERE play_count > 0 AND last_played_at IS NOT NULL AND last_played_at < ?1 ORDER BY last_played_at ASC LIMIT ?2",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![cutoff, limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Get random discovery songs (low play count, random order)
pub fn get_discovery_songs(
    conn: &Connection,
    max_play_count: i64,
    limit: u32,
) -> Result<Vec<DbSong>> {
    let sql = format!(
        "SELECT {} FROM songs WHERE play_count <= ?1 ORDER BY RANDOM() LIMIT ?2",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![max_play_count, limit], row_to_db_song)?
        .collect::<Result<Vec<_>>>()?;

    Ok(songs)
}

/// Get daily mix songs (weighted random based on date seed)
pub fn get_daily_mix(conn: &Connection, _seed: i64, limit: u32) -> Result<Vec<DbSong>> {
    // Use weighted scoring with RANDOM() for variety:
    // - Lower play count = higher score (discovery)
    // - Recently played = lower score (avoid repetition)
    // - Liked songs = strong preference signal
    // - HR/SQ = bonus
    // - RANDOM() adds variety each call

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let thirty_days_ago = now - 30 * 86400;

    let sql = format!(
        "SELECT {} FROM songs 
         ORDER BY 
           (10 - MIN(play_count, 10)) * 3.0 + 
           (CASE WHEN last_played_at IS NULL THEN 5.0 
                 WHEN last_played_at < ?1 THEN 3.0 
                 ELSE 0.0 END) +
           (CASE WHEN liked_at IS NOT NULL THEN 8.0 ELSE 0.0 END) +
           (CASE WHEN is_hr = 1 THEN 2.0 WHEN is_sq = 1 THEN 1.0 ELSE 0.0 END) +
           (ABS(RANDOM()) % 100) / 100.0
         DESC 
         LIMIT ?2",
        SELECT_COLUMNS
    );

    let mut stmt = conn.prepare(&sql)?;

    let songs = stmt
        .query_map(params![thirty_days_ago, limit], row_to_db_song)?
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
            "INSERT INTO songs
             (id, title, artist, album, duration, file_path, file_size,
              is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
              stream_info, file_modified, format, bit_depth, sample_rate, bitrate, channels,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                     COALESCE(?21, strftime('%s','now')),
                     strftime('%s','now'))
             ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              artist = excluded.artist,
              album = excluded.album,
              duration = excluded.duration,
              file_path = excluded.file_path,
              file_size = excluded.file_size,
              is_hr = excluded.is_hr,
              is_sq = excluded.is_sq,
              cover_hash = excluded.cover_hash,
              source_type = excluded.source_type,
              server_id = excluded.server_id,
              server_song_id = excluded.server_song_id,
              stream_info = excluded.stream_info,
              file_modified = excluded.file_modified,
              format = excluded.format,
              bit_depth = excluded.bit_depth,
              sample_rate = excluded.sample_rate,
              bitrate = excluded.bitrate,
              channels = excluded.channels,
              updated_at = excluded.updated_at"
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
                song.created_at,
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

/// Get existing `created_at` values for songs of a given source/server.
///
/// This is used to preserve "date added" across re-scans when we delete-and-reinsert
/// streaming libraries (so new songs sort after old ones).
pub fn get_created_ats_by_source(
    conn: &Connection,
    source_type: &str,
    server_id: Option<&str>,
) -> Result<HashMap<String, i64>> {
    let mut map = HashMap::new();

    if let Some(sid) = server_id {
        let mut stmt = conn.prepare(
            "SELECT id, created_at FROM songs
             WHERE source_type = ?1 AND server_id = ?2",
        )?;
        let rows = stmt.query_map(params![source_type, sid], |row| {
            let id: String = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            Ok((id, created_at))
        })?;

        for row in rows {
            let (id, created_at) = row?;
            map.insert(id, created_at);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, created_at FROM songs
             WHERE source_type = ?1",
        )?;
        let rows = stmt.query_map(params![source_type], |row| {
            let id: String = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            Ok((id, created_at))
        })?;

        for row in rows {
            let (id, created_at) = row?;
            map.insert(id, created_at);
        }
    }

    Ok(map)
}

/// Get user-owned song state that should survive delete-and-reinsert scans.
pub fn get_user_stats_by_source(
    conn: &Connection,
    source_type: &str,
    server_id: Option<&str>,
) -> Result<HashMap<String, SongUserStats>> {
    let mut map = HashMap::new();

    if let Some(sid) = server_id {
        let mut stmt = conn.prepare(
            "SELECT id, play_count, last_played_at, liked_at FROM songs
             WHERE source_type = ?1 AND server_id = ?2",
        )?;
        let rows = stmt.query_map(params![source_type, sid], |row| {
            let id: String = row.get(0)?;
            Ok((id, SongUserStats {
                play_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                last_played_at: row.get(2)?,
                liked_at: row.get(3)?,
            }))
        })?;

        for row in rows {
            let (id, stats) = row?;
            map.insert(id, stats);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, play_count, last_played_at, liked_at FROM songs
             WHERE source_type = ?1",
        )?;
        let rows = stmt.query_map(params![source_type], |row| {
            let id: String = row.get(0)?;
            Ok((id, SongUserStats {
                play_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                last_played_at: row.get(2)?,
                liked_at: row.get(3)?,
            }))
        })?;

        for row in rows {
            let (id, stats) = row?;
            map.insert(id, stats);
        }
    }

    Ok(map)
}

pub fn restore_user_stats(conn: &mut Connection, stats: &HashMap<String, SongUserStats>) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut affected = 0;

    {
        let mut stmt = tx.prepare(
            "UPDATE songs
             SET play_count = ?1, last_played_at = ?2, liked_at = ?3
             WHERE id = ?4",
        )?;

        for (id, item) in stats {
            affected += stmt.execute(params![
                item.play_count,
                item.last_played_at,
                item.liked_at,
                id,
            ])?;
        }
    }

    tx.commit()?;
    Ok(affected)
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
