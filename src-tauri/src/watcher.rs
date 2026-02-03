//! File system watcher for desktop platforms
//! Monitors music directories for changes and triggers incremental scans.

#[cfg(desktop)]
pub mod desktop {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use tauri::{AppHandle, Emitter, Manager};

    use crate::commands::CoverCacheState;
    use crate::db::{self, DbState, SongInput};
    use crate::utils::audio;
    use crate::utils::cover::extract_and_cache_cover;

    /// Shared state for the file watcher
    pub struct WatcherState {
        watcher: Option<RecommendedWatcher>,
        watched_dirs: Vec<String>,
    }

    impl WatcherState {
        pub fn new() -> Self {
            Self {
                watcher: None,
                watched_dirs: Vec::new(),
            }
        }
    }

    /// Managed Tauri state wrapper
    pub struct FileWatcherState(pub Mutex<WatcherState>);

    /// Start watching directories for file changes
    pub fn start_watching(
        app_handle: &AppHandle,
        directories: Vec<String>,
    ) -> Result<(), String> {
        let watcher_state: tauri::State<'_, FileWatcherState> = app_handle.state();

        let mut state = watcher_state
            .0
            .lock()
            .map_err(|e| format!("Failed to lock watcher state: {}", e))?;

        // Stop existing watcher if any
        state.watcher = None;
        state.watched_dirs.clear();

        if directories.is_empty() {
            return Ok(());
        }

        // Debounce state: collect changed paths, process after 500ms of quiet
        let pending_paths: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let last_event_time: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
        let app_for_debounce = app_handle.clone();
        let pending_for_debounce = pending_paths.clone();
        let last_time_for_debounce = last_event_time.clone();

        // Spawn debounce processor thread
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(500));

                let should_process = {
                    let last = last_time_for_debounce.lock().unwrap();
                    let pending = pending_for_debounce.lock().unwrap();
                    !pending.is_empty() && last.elapsed() >= Duration::from_millis(500)
                };

                if should_process {
                    let paths: Vec<PathBuf> = {
                        let mut pending = pending_for_debounce.lock().unwrap();
                        let collected: Vec<PathBuf> = pending.drain().collect();
                        collected
                    };

                    if !paths.is_empty() {
                        process_changed_files(&app_for_debounce, &paths);
                    }
                }
            }
        });

        // Create the file watcher
        let pending_for_handler = pending_paths;
        let last_time_for_handler = last_event_time;

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        let audio_paths: Vec<PathBuf> = event
                            .paths
                            .into_iter()
                            .filter(|p| p.is_file() && audio::is_audio_file(p) || !p.exists())
                            .collect();

                        if !audio_paths.is_empty() {
                            if let Ok(mut pending) = pending_for_handler.lock() {
                                for p in audio_paths {
                                    pending.insert(p);
                                }
                            }
                            if let Ok(mut last) = last_time_for_handler.lock() {
                                *last = Instant::now();
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| format!("Failed to create file watcher: {}", e))?;

        state.watcher = Some(watcher);

        // Watch each directory
        for dir in &directories {
            let path = PathBuf::from(dir);
            if path.exists() && path.is_dir() {
                if let Some(ref mut w) = state.watcher {
                    w.watch(&path, RecursiveMode::Recursive)
                        .map_err(|e| format!("Failed to watch {}: {}", dir, e))?;
                }
            }
        }

        state.watched_dirs = directories;
        Ok(())
    }

    /// Stop watching all directories
    pub fn stop_watching(app_handle: &AppHandle) -> Result<(), String> {
        let watcher_state: tauri::State<'_, FileWatcherState> = app_handle.state();

        let mut state = watcher_state
            .0
            .lock()
            .map_err(|e| format!("Failed to lock watcher state: {}", e))?;

        state.watcher = None;
        state.watched_dirs.clear();
        Ok(())
    }

    /// Process changed files: mini incremental scan
    fn process_changed_files(app_handle: &AppHandle, paths: &[PathBuf]) {
        let db_state: tauri::State<'_, DbState> = app_handle.state();
        let cover_cache_state: tauri::State<'_, CoverCacheState> = app_handle.state();

        // Get cover cache for processing
        let cover_cache = match cover_cache_state.0.lock() {
            Ok(c) => c.clone_arc(),
            Err(_) => return,
        };

        // Separate existing files from deleted files
        let mut to_scan: Vec<&PathBuf> = Vec::new();
        let mut to_delete: Vec<String> = Vec::new();

        for path in paths {
            if path.exists() && path.is_file() && audio::is_audio_file(path) {
                to_scan.push(path);
            } else if !path.exists() {
                // File was deleted
                to_delete.push(path.to_string_lossy().to_string());
            }
        }

        let mut changed = false;

        // Scan new/modified files
        if !to_scan.is_empty() {
            let song_inputs: Vec<SongInput> = to_scan
                .iter()
                .filter_map(|path| {
                    audio::read_metadata_with_mtime(path).ok().map(|song| {
                        // Extract and cache cover
                        let cover_hash = extract_and_cache_cover(path, &cover_cache).ok().flatten();
                        SongInput {
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
                        }
                    })
                })
                .collect();

            if !song_inputs.is_empty() {
                if let Ok(mut conn) = db_state.0.lock() {
                    let _ = db::songs::save_songs(&mut conn, &song_inputs, "local", None);
                    changed = true;
                }
            }
        }

        // Delete removed files from DB
        if !to_delete.is_empty() {
            if let Ok(conn) = db_state.0.lock() {
                for path_str in &to_delete {
                    let _ = conn.execute(
                        "DELETE FROM songs WHERE file_path = ?1 AND source_type = 'local'",
                        [path_str],
                    );
                }
                changed = true;
            }
        }

        // Notify frontend
        if changed {
            let _ = app_handle.emit("library-updated", ());
        }
    }
}
