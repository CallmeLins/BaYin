//! Stream (remote) playlists cache.

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStreamPlaylist {
    pub server_id: String,
    pub playlist_id: String,
    pub name: String,
    pub song_count: i64,
    pub kind: String,
    pub updated_at: Option<i64>,
    pub synced_at: i64,
}

/// Get cached playlists for a server.
pub fn get_stream_playlists(conn: &Connection, server_id: &str) -> Result<Vec<DbStreamPlaylist>> {
    let mut stmt = conn.prepare(
        "SELECT server_id, playlist_id, name, song_count, kind, updated_at, synced_at
         FROM stream_playlists
         WHERE server_id = ?1
         ORDER BY name COLLATE NOCASE",
    )?;

    let playlists = stmt
        .query_map(params![server_id], |row| {
            Ok(DbStreamPlaylist {
                server_id: row.get(0)?,
                playlist_id: row.get(1)?,
                name: row.get(2)?,
                song_count: row.get(3)?,
                kind: row.get(4)?,
                updated_at: row.get(5)?,
                synced_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(playlists)
}

pub fn get_stream_playlists_last_sync(conn: &Connection, server_id: &str) -> Result<Option<i64>> {
    conn.query_row(
        "SELECT MAX(synced_at) FROM stream_playlists WHERE server_id = ?1",
        params![server_id],
        |row| row.get(0),
    )
}

/// Replace playlists cache for a server (transaction).
pub fn replace_stream_playlists(
    conn: &mut Connection,
    server_id: &str,
    playlists: &[DbStreamPlaylist],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM stream_playlists WHERE server_id = ?1",
        params![server_id],
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO stream_playlists
             (server_id, playlist_id, name, song_count, kind, updated_at, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for p in playlists {
            stmt.execute(params![
                server_id,
                p.playlist_id,
                p.name,
                p.song_count,
                p.kind,
                p.updated_at,
                p.synced_at
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn get_stream_playlist_items_last_sync(
    conn: &Connection,
    server_id: &str,
    playlist_id: &str,
) -> Result<Option<i64>> {
    conn.query_row(
        "SELECT MAX(synced_at) FROM stream_playlist_items
         WHERE server_id = ?1 AND playlist_id = ?2",
        params![server_id, playlist_id],
        |row| row.get(0),
    )
}

pub fn get_stream_playlist_items(
    conn: &Connection,
    server_id: &str,
    playlist_id: &str,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT song_id
         FROM stream_playlist_items
         WHERE server_id = ?1 AND playlist_id = ?2
         ORDER BY position ASC",
    )?;

    let items = stmt
        .query_map(params![server_id, playlist_id], |row| row.get(0))?
        .collect::<Result<Vec<String>>>()?;

    Ok(items)
}

pub fn replace_stream_playlist_items(
    conn: &mut Connection,
    server_id: &str,
    playlist_id: &str,
    song_ids: &[String],
    synced_at: i64,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM stream_playlist_items WHERE server_id = ?1 AND playlist_id = ?2",
        params![server_id, playlist_id],
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO stream_playlist_items
             (server_id, playlist_id, position, song_id, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;

        for (idx, song_id) in song_ids.iter().enumerate() {
            stmt.execute(params![server_id, playlist_id, idx as i64, song_id, synced_at])?;
        }
    }

    tx.commit()?;
    Ok(())
}

