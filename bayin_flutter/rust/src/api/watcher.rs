use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWatchEvent {
    pub path: String,
    pub kind: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWatcherStatus {
    pub running: bool,
    pub watched_dirs: Vec<String>,
    pub pending_events: usize,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod platform_impl {
    use super::{FileWatchEvent, FileWatcherStatus};
    use crate::db::{self, SongInput};
    use crate::state::{with_cover_cache, with_db_mut};
    use crate::utils::audio;
    use crate::utils::cover::extract_and_cache_cover;
    use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use rusqlite::params;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct FileWatcherRuntime {
        _watcher: RecommendedWatcher,
        events: Arc<Mutex<Vec<FileWatchEvent>>>,
        watched_dirs: Vec<String>,
        stop_flag: Arc<AtomicBool>,
    }

    static FILE_WATCHER_RUNTIME: LazyLock<Mutex<Option<FileWatcherRuntime>>> =
        LazyLock::new(|| Mutex::new(None));

    pub fn start_file_watcher(directories: Vec<String>) -> Result<(), String> {
        let mut runtime_slot = FILE_WATCHER_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock watcher runtime: {err}"))?;

        if let Some(existing) = runtime_slot.as_ref() {
            existing.stop_flag.store(true, Ordering::Relaxed);
        }
        *runtime_slot = None;

        if directories.is_empty() {
            return Ok(());
        }

        let events = Arc::new(Mutex::new(Vec::<FileWatchEvent>::new()));
        let pending_map = Arc::new(Mutex::new(HashMap::<PathBuf, String>::new()));
        let last_event_time = Arc::new(Mutex::new(Instant::now()));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let pending_for_handler = Arc::clone(&pending_map);
        let last_for_handler = Arc::clone(&last_event_time);

        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let Ok(event) = result else {
                    return;
                };
                let kind = map_event_kind(&event.kind);
                let Some(kind) = kind else {
                    return;
                };

                if let Ok(mut pending) = pending_for_handler.lock() {
                    for path in event.paths {
                        let is_deleted = !path.exists();
                        let is_audio = path.is_file() && audio::is_audio_file(&path);
                        if is_audio || is_deleted {
                            pending.insert(path, kind.to_string());
                        }
                    }
                }
                if let Ok(mut last) = last_for_handler.lock() {
                    *last = Instant::now();
                }
            })
            .map_err(|err| format!("Failed to create watcher: {err}"))?;

        let mut watched_dirs = Vec::<String>::new();
        for dir in directories {
            let path = PathBuf::from(&dir);
            if !path.exists() || !path.is_dir() {
                continue;
            }
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|err| format!("Failed to watch '{dir}': {err}"))?;
            watched_dirs.push(dir);
        }

        let events_for_worker = Arc::clone(&events);
        let pending_for_worker = Arc::clone(&pending_map);
        let last_for_worker = Arc::clone(&last_event_time);
        let stop_for_worker = Arc::clone(&stop_flag);

        std::thread::spawn(move || loop {
            if stop_for_worker.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));

            let should_process = {
                let pending_non_empty = pending_for_worker
                    .lock()
                    .map(|pending| !pending.is_empty())
                    .unwrap_or(false);
                let elapsed_ready = last_for_worker
                    .lock()
                    .map(|last| last.elapsed() >= Duration::from_millis(500))
                    .unwrap_or(false);
                pending_non_empty && elapsed_ready
            };
            if !should_process {
                continue;
            }

            let drained: Vec<(PathBuf, String)> = {
                let mut pending = match pending_for_worker.lock() {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                pending.drain().collect()
            };

            if drained.is_empty() {
                continue;
            }

            let changed = process_changed_files(drained.iter().map(|(path, _)| path).collect());
            if !changed {
                continue;
            }

            let now = now_millis();
            let mut output = Vec::with_capacity(drained.len());
            for (path, kind) in drained {
                output.push(FileWatchEvent {
                    path: path.to_string_lossy().to_string(),
                    kind,
                    timestamp_ms: now,
                });
            }

            if let Ok(mut queue) = events_for_worker.lock() {
                queue.extend(output);
                if queue.len() > 512 {
                    let overflow = queue.len() - 512;
                    queue.drain(0..overflow);
                }
            }
        });

        *runtime_slot = Some(FileWatcherRuntime {
            _watcher: watcher,
            events,
            watched_dirs,
            stop_flag,
        });
        Ok(())
    }

    pub fn stop_file_watcher() -> Result<(), String> {
        let mut runtime_slot = FILE_WATCHER_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock watcher runtime: {err}"))?;
        if let Some(runtime) = runtime_slot.as_ref() {
            runtime.stop_flag.store(true, Ordering::Relaxed);
        }
        *runtime_slot = None;
        Ok(())
    }

    pub fn poll_file_watcher_events(max_events: usize) -> Result<Vec<FileWatchEvent>, String> {
        let mut runtime_slot = FILE_WATCHER_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock watcher runtime: {err}"))?;
        let Some(runtime) = runtime_slot.as_mut() else {
            return Ok(Vec::new());
        };

        let mut queue = runtime
            .events
            .lock()
            .map_err(|err| format!("Failed to lock watcher queue: {err}"))?;
        if queue.is_empty() {
            return Ok(Vec::new());
        }

        let take = max_events.clamp(1, 512).min(queue.len());
        Ok(queue.drain(0..take).collect())
    }

    pub fn file_watcher_status() -> Result<FileWatcherStatus, String> {
        let runtime_slot = FILE_WATCHER_RUNTIME
            .lock()
            .map_err(|err| format!("Failed to lock watcher runtime: {err}"))?;
        let Some(runtime) = runtime_slot.as_ref() else {
            return Ok(FileWatcherStatus {
                running: false,
                watched_dirs: Vec::new(),
                pending_events: 0,
            });
        };

        let pending = runtime
            .events
            .lock()
            .map_err(|err| format!("Failed to lock watcher queue: {err}"))?
            .len();

        Ok(FileWatcherStatus {
            running: true,
            watched_dirs: runtime.watched_dirs.clone(),
            pending_events: pending,
        })
    }

    fn process_changed_files(paths: Vec<&PathBuf>) -> bool {
        let mut to_upsert = Vec::<SongInput>::new();
        let mut to_delete = Vec::<String>::new();

        for path in paths {
            let is_deleted = !path.exists();
            if is_deleted {
                to_delete.push(path.to_string_lossy().to_string());
                continue;
            }

            if !path.is_file() || !audio::is_audio_file(path) {
                continue;
            }

            let Ok(song) = audio::read_metadata_with_mtime(path) else {
                continue;
            };
            let cover_hash = with_cover_cache(|cache| extract_and_cache_cover(path, cache))
                .ok()
                .flatten();
            to_upsert.push(SongInput {
                id: song.id,
                title: song.title,
                artist: song.artist,
                album: song.album,
                duration: song.duration,
                file_path: song.file_path,
                file_size: song.file_size as i64,
                is_hr: song.is_hr,
                is_sq: song.is_sq,
                cover_hash,
                server_song_id: None,
                stream_info: None,
                file_modified: Some(song.file_modified),
                format: song.format,
                bit_depth: song.bit_depth,
                sample_rate: song.sample_rate,
                bitrate: song.bitrate,
                channels: song.channels,
                created_at: None,
            });
        }

        let write_result = with_db_mut(|conn| {
            let mut changed = 0usize;

            if !to_upsert.is_empty() {
                let saved = db::songs::save_songs(conn, &to_upsert, "local", None)
                    .map_err(|err| err.to_string())?;
                changed += saved;
            }

            for file_path in &to_delete {
                let deleted = conn
                    .execute(
                        "DELETE FROM songs WHERE file_path = ?1 AND source_type = 'local'",
                        params![file_path],
                    )
                    .map_err(|err| err.to_string())?;
                changed += deleted;
            }

            Ok(changed)
        });

        write_result.map(|count| count > 0).unwrap_or(false)
    }

    fn map_event_kind(kind: &EventKind) -> Option<&'static str> {
        match kind {
            EventKind::Create(_) => Some("create"),
            EventKind::Modify(_) => Some("modify"),
            EventKind::Remove(_) => Some("remove"),
            _ => None,
        }
    }

    fn now_millis() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|v| v.as_millis() as i64)
            .unwrap_or(0)
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod platform_impl {
    use super::{FileWatchEvent, FileWatcherStatus};

    pub fn start_file_watcher(_directories: Vec<String>) -> Result<(), String> {
        Ok(())
    }

    pub fn stop_file_watcher() -> Result<(), String> {
        Ok(())
    }

    pub fn poll_file_watcher_events(_max_events: usize) -> Result<Vec<FileWatchEvent>, String> {
        Ok(Vec::new())
    }

    pub fn file_watcher_status() -> Result<FileWatcherStatus, String> {
        Ok(FileWatcherStatus {
            running: false,
            watched_dirs: Vec::new(),
            pending_events: 0,
        })
    }
}

pub use platform_impl::{
    file_watcher_status, poll_file_watcher_events, start_file_watcher, stop_file_watcher,
};
