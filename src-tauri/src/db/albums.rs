//! Album and artist aggregation queries

use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};

/// Aggregated album data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbAlbum {
    pub id: String,
    pub name: String,
    pub artist: String,
    pub cover_hash: Option<String>,  // SHA256 hash for cover lookup
    pub song_count: i64,
}

/// Aggregated artist data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbArtist {
    pub id: String,
    pub name: String,
    pub cover_hash: Option<String>,  // SHA256 hash for cover lookup
    pub song_count: i64,
}

/// Get all albums aggregated from songs
pub fn get_all_albums(conn: &Connection) -> Result<Vec<DbAlbum>> {
    let mut stmt = conn.prepare(
        "SELECT
            album,
            MIN(artist) as artist,
            MIN(cover_hash) as cover_hash,
            COUNT(*) as song_count
         FROM songs
         WHERE cover_hash IS NOT NULL OR cover_hash IS NULL
         GROUP BY album
         ORDER BY album COLLATE NOCASE"
    )?;

    let albums = stmt.query_map([], |row| {
        let album_name: String = row.get(0)?;
        let artist: String = row.get(1)?;
        let cover_hash: Option<String> = row.get(2)?;
        let song_count: i64 = row.get(3)?;

        // Generate a stable ID from album name
        let id = format!("album-{:x}", md5::compute(&album_name));

        Ok(DbAlbum {
            id,
            name: album_name,
            artist,
            cover_hash,
            song_count,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(albums)
}

/// Get all artists aggregated from songs
pub fn get_all_artists(conn: &Connection) -> Result<Vec<DbArtist>> {
    let mut stmt = conn.prepare(
        "SELECT
            artist,
            MIN(cover_hash) as cover_hash,
            COUNT(*) as song_count
         FROM songs
         GROUP BY artist
         ORDER BY artist COLLATE NOCASE"
    )?;

    let artists = stmt.query_map([], |row| {
        let artist_name: String = row.get(0)?;
        let cover_hash: Option<String> = row.get(1)?;
        let song_count: i64 = row.get(2)?;

        // Generate a stable ID from artist name
        let id = format!("artist-{:x}", md5::compute(&artist_name));

        Ok(DbArtist {
            id,
            name: artist_name,
            cover_hash,
            song_count,
        })
    })?.collect::<Result<Vec<_>>>()?;

    Ok(artists)
}

/// Get songs for a specific album
pub fn get_songs_by_album(conn: &Connection, album: &str) -> Result<Vec<super::DbSong>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album, duration, file_path, file_size,
                is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
                stream_info, file_modified
         FROM songs
         WHERE album = ?1
         ORDER BY title COLLATE NOCASE"
    )?;

    let songs = stmt.query_map([album], |row| {
        Ok(super::DbSong {
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

/// Get songs for a specific artist
pub fn get_songs_by_artist(conn: &Connection, artist: &str) -> Result<Vec<super::DbSong>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, artist, album, duration, file_path, file_size,
                is_hr, is_sq, cover_hash, source_type, server_id, server_song_id,
                stream_info, file_modified
         FROM songs
         WHERE artist = ?1
         ORDER BY album COLLATE NOCASE, title COLLATE NOCASE"
    )?;

    let songs = stmt.query_map([artist], |row| {
        Ok(super::DbSong {
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
