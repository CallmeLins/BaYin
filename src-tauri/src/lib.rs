mod audio_engine;
mod commands;
mod playback;
mod playback_control;
mod db;
mod models;
mod utils;
mod watcher;

#[cfg(target_os = "android")]
mod system_media_android;

use commands::{
    audio_enable_visualization,
    audio_get_state,
    audio_pause,
    // Audio engine commands
    audio_play,
    audio_resume,
    audio_seek,
    audio_set_eq_bands,
    audio_set_eq_enabled,
    audio_set_volume,
    audio_stop,
    playback_next,
    playback_play_index,
    playback_previous,
    playback_set_mode,
    playback_set_queue,
    cleanup_missing_songs,
    cleanup_orphaned_covers,
    clear_cover_cache,
    db_clear_all_songs,
    db_clear_scan_config,
    db_clear_stream_servers,
    db_delete_songs_by_source,
    db_delete_stream_server,
    db_get_all_albums,
    db_get_all_artists,
    db_get_all_songs,
    db_get_library_stats,
    db_get_scan_config,
    db_get_stream_servers,
    db_migrate_from_localstorage,
    db_save_scan_config,
    db_save_songs,
    db_save_stream_server,
    fetch_stream_songs,
    fetch_subsonic_songs,
    get_cover_cache_stats,
    // Cover cache commands
    get_cover_url,
    get_cover_urls_batch,
    get_lyrics,
    get_music_metadata,
    get_stream_lyrics,
    get_stream_url,
    get_subsonic_lyrics,
    get_subsonic_stream_url,
    jellyfin_authenticate,
    list_directories,
    scan_local_to_db,
    scan_music_files,
    scan_stream_to_db,
    stream_get_playlist_tracks,
    stream_get_playlists,
    // File watcher commands
    start_file_watcher,
    stop_file_watcher,
    test_stream_connection,
    test_subsonic_connection,
    CoverCacheState,
};
use db::DbState;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::sync::Mutex;
#[cfg(desktop)]
use std::sync::OnceLock;
use tauri::{Emitter, Manager};
use utils::cover::CoverCache;

#[cfg(desktop)]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
#[cfg(desktop)]
use tauri::tray::TrayIconBuilder;

#[cfg(desktop)]
#[derive(Clone, serde::Serialize)]
struct TrayCommandPayload {
    command: String,
}

#[cfg(desktop)]
#[derive(Clone)]
struct TrayMenuState {
    lang: String,
    muted: bool,
}

#[cfg(desktop)]
static TRAY_MENU_STATE: OnceLock<Mutex<TrayMenuState>> = OnceLock::new();

#[cfg(desktop)]
fn tray_menu_state() -> &'static Mutex<TrayMenuState> {
    TRAY_MENU_STATE.get_or_init(|| Mutex::new(TrayMenuState {
        lang: "zh-CN".to_string(),
        muted: false,
    }))
}

#[cfg(desktop)]
fn build_tray_menu(app: &tauri::AppHandle, lang: &str, muted: bool) -> Menu<tauri::Wry> {
    let zh = lang == "zh-CN";

    let (play_pause_label, prev_label, next_label, show_label, exit_label) = if zh {
        ("播放/暂停", "上一曲", "下一曲", "打开主窗口", "退出")
    } else {
        ("Play/Pause", "Previous", "Next", "Show Window", "Exit")
    };

    let toggle_mute_label = if zh {
        if muted { "关闭静音" } else { "静音" }
    } else {
        if muted { "Unmute" } else { "Mute" }
    };

    let play_pause_item =
        MenuItem::with_id(app, "play_pause", play_pause_label, true, None::<&str>).unwrap();
    let prev_item = MenuItem::with_id(app, "previous", prev_label, true, None::<&str>).unwrap();
    let next_item = MenuItem::with_id(app, "next", next_label, true, None::<&str>).unwrap();
    // Single toggle item: label reflects the *action*.
    let toggle_mute_item =
        MenuItem::with_id(app, "toggle_mute", toggle_mute_label, true, None::<&str>).unwrap();
    let show_item = MenuItem::with_id(app, "show", show_label, true, None::<&str>).unwrap();
    let exit_item = MenuItem::with_id(app, "exit", exit_label, true, None::<&str>).unwrap();

    Menu::with_items(
        app,
        &[
            &play_pause_item,
            &prev_item,
            &next_item,
            &PredefinedMenuItem::separator(app).unwrap(),
            &toggle_mute_item,
            &PredefinedMenuItem::separator(app).unwrap(),
            &show_item,
            &exit_item,
        ],
    )
    .unwrap()
}

#[cfg(desktop)]
fn refresh_tray_menu(app: &tauri::AppHandle) {
    let state = match tray_menu_state().lock() {
        Ok(s) => s.clone(),
        Err(_) => return,
    };

    if let Some(tray) = app.tray_by_id("main-tray") {
        let menu = build_tray_menu(app, &state.lang, state.muted);
        let _ = tray.set_menu(Some(menu));
    }
}

#[cfg(desktop)]
#[tauri::command]
fn set_tray_language(app: tauri::AppHandle, lang: String) {
    if let Ok(mut state) = tray_menu_state().lock() {
        state.lang = lang;
    }
    refresh_tray_menu(&app);
}

#[cfg(desktop)]
#[tauri::command]
fn set_tray_muted(app: tauri::AppHandle, muted: bool) {
    if let Ok(mut state) = tray_menu_state().lock() {
        state.muted = muted;
    }
    refresh_tray_menu(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(bayin_system_media::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init());

    // 窗口状态插件仅桌面端使用（必须在窗口创建前注册）
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

    builder
        .invoke_handler(tauri::generate_handler![
            scan_music_files,
            get_music_metadata,
            get_lyrics,
            list_directories,
            // 统一流媒体命令
            test_stream_connection,
            fetch_stream_songs,
            get_stream_url,
            get_stream_lyrics,
            jellyfin_authenticate,
            // Subsonic API 命令
            test_subsonic_connection,
            fetch_subsonic_songs,
            get_subsonic_stream_url,
            get_subsonic_lyrics,
            // 数据库命令
            db_get_all_songs,
            db_get_all_albums,
            db_get_all_artists,
            db_save_songs,
            db_delete_songs_by_source,
            db_clear_all_songs,
            db_get_stream_servers,
            db_save_stream_server,
            db_delete_stream_server,
            db_clear_stream_servers,
            db_save_scan_config,
            db_get_scan_config,
            db_clear_scan_config,
            db_migrate_from_localstorage,
            db_get_library_stats,
            // 高级扫描命令
            scan_local_to_db,
            scan_stream_to_db,
            // 封面缓存命令
            get_cover_url,
            get_cover_urls_batch,
            get_cover_cache_stats,
            cleanup_orphaned_covers,
            clear_cover_cache,
            cleanup_missing_songs,
            // 文件监听命令
            start_file_watcher,
            stop_file_watcher,
            // 托盘命令
            #[cfg(desktop)]
            set_tray_language,
            #[cfg(desktop)]
            set_tray_muted,
            // 音频引擎命令
            audio_play,
            audio_pause,
            audio_resume,
            audio_stop,
            audio_seek,
            audio_set_volume,
            audio_set_eq_bands,
            audio_set_eq_enabled,
            audio_enable_visualization,
            audio_get_state
            ,
            // Playback domain commands (queue + cross-platform control)
            playback_set_queue,
            playback_set_mode,
            playback_play_index,
            playback_next,
            playback_previous,
            // Stream playlists (remote)
            stream_get_playlists,
            stream_get_playlist_tracks
        ])
        .on_window_event(|_window, _event| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = _event {
                api.prevent_close();
                let _ = _window.hide();
            }
        })
        .setup(|app| {
            // 初始化数据库
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            // 确保目录存在
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            let db_path = app_data_dir.join("bayin.db");
            let conn = db::open_db(&db_path).expect("Failed to open database");

            app.manage(DbState(Mutex::new(conn)));

            // 初始化封面缓存
            let cache_dir = app
                .path()
                .app_cache_dir()
                .expect("Failed to get app cache directory");
            let cover_cache_dir = cache_dir.join("covers");
            let cover_cache = CoverCache::new(cover_cache_dir);
            cover_cache.ensure_dirs().expect("Failed to create cover cache directories");

            app.manage(CoverCacheState(Mutex::new(cover_cache)));

            // 初始化文件监听器状态（仅桌面端）
            #[cfg(desktop)]
            {
                use watcher::desktop::{FileWatcherState, WatcherState};
                app.manage(FileWatcherState(Mutex::new(WatcherState::new())));
            }

            // 初始化音频引擎
            {
                use audio_engine::engine::AudioEngine;
                let audio_engine = AudioEngine::new(app.handle().clone());
                app.manage(audio_engine::AudioEngineState::new(audio_engine));
            }

            // Playback domain state (queue + play mode)
            {
                app.manage(playback::PlaybackDomainState::new());
            }

            #[cfg(target_os = "android")]
            {
                // Only expose the AppHandle to JNI after all required states are managed,
                // otherwise background threads (MediaPlaybackService) can crash by calling `state()` too early.
                system_media_android::set_app_handle(app.handle().clone());
            }

            // 桌面端：创建系统托盘
            #[cfg(desktop)]
            {
                let app_handle = app.handle().clone();
                // Default tray language is Chinese; frontend can call `set_tray_language` to update.
                // Also initialize the shared state used for "mute/unmute" pair enabling.
                {
                    let _ = tray_menu_state();
                }
                let menu = build_tray_menu(&app_handle, "zh-CN", false);

                TrayIconBuilder::with_id("main-tray")
                    .icon(app.default_window_icon().cloned().expect("no app icon"))
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "play_pause" => {
                            let _ = app.emit(
                                "tray:command",
                                TrayCommandPayload {
                                    command: "toggle_play".to_string(),
                                },
                            );
                        }
                        "previous" => {
                            let _ = app.emit(
                                "tray:command",
                                TrayCommandPayload {
                                    command: "previous".to_string(),
                                },
                            );
                        }
                        "next" => {
                            let _ = app.emit(
                                "tray:command",
                                TrayCommandPayload {
                                    command: "next".to_string(),
                                },
                            );
                        }
                        "toggle_mute" => {
                            let muted = if let Ok(mut state) = tray_menu_state().lock() {
                                state.muted = !state.muted;
                                state.muted
                            } else {
                                false
                            };

                            // Refresh immediately so label flips right away.
                            refresh_tray_menu(app);

                            let _ = app.emit(
                                "tray:command",
                                TrayCommandPayload {
                                    command: (if muted { "mute" } else { "unmute" }).to_string(),
                                },
                            );
                        }
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "exit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            // 桌面端：窗口状态已恢复，显示窗口
            #[cfg(desktop)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.as_ref().window().show();
                }
            }

            // 启动后台增量扫描（延迟启动，等前端初始化完成）
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                // Wait 500ms for frontend to initialize and load cached data from DB
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Read scan config from DB
                let db_state: tauri::State<'_, DbState> = app_handle.state();
                let scan_config = {
                    let conn = match db_state.0.lock() {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    db::servers::get_scan_config(&conn).ok().flatten()
                };

                if let Some(config) = scan_config {
                    if !config.directories.is_empty() {
                        #[cfg(desktop)]
                        let watch_dirs = config.directories.clone();
                        // Run incremental local scan
                        let options = models::LocalScanOptions {
                            directories: config.directories,
                            mode: models::ScanMode::Incremental,
                            min_duration: if config.skip_short { Some(config.min_duration) } else { None },
                            batch_size: 500,
                        };

                        // Run incremental local scan. Keep it isolated so early returns don't prevent starting the watcher.
                        (|| {
                            let db_state2: tauri::State<'_, DbState> = app_handle.state();
                            // Collect files
                            let mut audio_paths = Vec::new();
                            for dir in &options.directories {
                                let dir_path = std::path::Path::new(dir);
                                if !dir_path.exists() {
                                    continue;
                                }
                                for entry in walkdir::WalkDir::new(dir_path)
                                    .follow_links(true)
                                    .into_iter()
                                    .filter_map(|e| e.ok())
                                {
                                    let path = entry.path();
                                    if path.is_file() && utils::audio::is_audio_file(path) {
                                        audio_paths.push(path.to_path_buf());
                                    }
                                }
                            }

                            // Check for changes (incremental)
                            let existing_files: std::collections::HashMap<String, Option<i64>> = {
                                let conn = match db_state2.0.lock() {
                                    Ok(c) => c,
                                    Err(_) => return,
                                };
                                let songs = db::songs::get_all_songs(&conn).unwrap_or_default();
                                songs
                                    .into_iter()
                                    .filter(|s| s.source_type == "local")
                                    .map(|s| (s.file_path, s.file_modified))
                                    .collect()
                            };

                            let min_dur = options.min_duration.unwrap_or(0.0);
                            let mut new_or_changed = Vec::new();

                            for path in &audio_paths {
                                let path_str = path.to_string_lossy().to_string();
                                let needs_scan = match existing_files.get(&path_str) {
                                    Some(Some(db_mtime)) => {
                                        match std::fs::metadata(path) {
                                            Ok(meta) => match meta.modified() {
                                                Ok(mtime) => {
                                                    let file_mtime = mtime
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .map(|d| d.as_secs() as i64)
                                                        .unwrap_or(0);
                                                    file_mtime > *db_mtime
                                                }
                                                Err(_) => true,
                                            },
                                            Err(_) => true,
                                        }
                                    }
                                    _ => true,
                                };

                                if needs_scan {
                                    new_or_changed.push(path.clone());
                                }
                            }

                            // Only proceed if there are changes or deleted files
                            let disk_paths: std::collections::HashSet<String> = audio_paths
                                .iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect();
                            let deleted_ids: Vec<String> = existing_files
                                .keys()
                                .filter(|k| !disk_paths.contains(k.as_str()))
                                .cloned()
                                .collect();

                            if new_or_changed.is_empty() && deleted_ids.is_empty() {
                                return; // No changes, skip
                            }

                            // Get cover cache for use in parallel processing
                            let cover_cache_state: tauri::State<'_, CoverCacheState> = app_handle.state();
                            let cover_cache = match cover_cache_state.0.lock() {
                                Ok(c) => c.clone_arc(),
                                Err(_) => return,
                            };

                            // Scan new/changed files
                            let song_inputs: Vec<db::SongInput> = new_or_changed
                                .par_iter()
                                .filter_map(|path| {
                                    match utils::audio::read_metadata_with_mtime(path) {
                                        Ok(song) => {
                                            if min_dur > 0.0 && song.duration < min_dur {
                                                return None;
                                            }
                                            // Extract and cache cover
                                            let cover_hash = utils::cover::extract_and_cache_cover(path, &cover_cache).ok().flatten();
                                            Some(db::SongInput {
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
                                            })
                                        }
                                        Err(_) => None,
                                    }
                                })
                                .collect();

                            // Write to DB
                            {
                                let mut conn = match db_state2.0.lock() {
                                    Ok(c) => c,
                                    Err(_) => return,
                                };
                                // Save new/changed songs
                                if !song_inputs.is_empty() {
                                    let _ = db::songs::save_songs(&mut conn, &song_inputs, "local", None);
                                }
                                // Delete removed files
                                for id in &deleted_ids {
                                    let _ = conn.execute("DELETE FROM songs WHERE file_path = ?1 AND source_type = 'local'", [id]);
                                }
                            }

                            // Emit library-updated event
                            if !song_inputs.is_empty() || !deleted_ids.is_empty() {
                                let _ = app_handle.emit("library-updated", ());
                            }
                        })();

                        // Start file watcher after scan completes (desktop only)
                        #[cfg(desktop)]
                        {
                            let _ = watcher::desktop::start_watching(&app_handle, watch_dirs);
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
