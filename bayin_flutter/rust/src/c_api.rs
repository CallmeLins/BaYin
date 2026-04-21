use std::ffi::{c_char, CStr, CString};
use std::sync::{LazyLock, Mutex};

use crate::api;
use crate::api::streaming::StreamPlaylistSongsRequest;
use crate::db::StreamServerInput;

static LAST_ERROR: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

fn set_last_error(message: impl Into<String>) {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = Some(message.into());
    }
}

fn clear_last_error() {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = None;
    }
}

fn into_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(value) => value.into_raw(),
        Err(_) => {
            set_last_error("CString conversion failed");
            std::ptr::null_mut()
        }
    }
}

fn ok_string(value: Result<String, String>) -> *mut c_char {
    match value {
        Ok(value) => {
            clear_last_error();
            into_c_string(value)
        }
        Err(err) => {
            set_last_error(err);
            std::ptr::null_mut()
        }
    }
}

fn read_string_arg(input: *const c_char) -> Result<String, String> {
    if input.is_null() {
        return Err("Received null string pointer".to_string());
    }

    let value = unsafe { CStr::from_ptr(input) };
    value
        .to_str()
        .map(|text| text.to_string())
        .map_err(|err| format!("Invalid UTF-8 string: {err}"))
}

fn handle_bool_result(result: Result<(), String>) -> bool {
    match result {
        Ok(()) => {
            clear_last_error();
            true
        }
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn bayin_ping() -> *mut c_char {
    ok_string(Ok(api::simple::ping()))
}

#[no_mangle]
pub extern "C" fn bayin_last_error_message() -> *mut c_char {
    let message = LAST_ERROR
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
        .unwrap_or_default();

    if message.is_empty() {
        return std::ptr::null_mut();
    }

    into_c_string(message)
}

#[no_mangle]
pub unsafe extern "C" fn bayin_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    let _ = unsafe { CString::from_raw(ptr) };
}

#[no_mangle]
pub extern "C" fn bayin_init_db(db_path: *const c_char) -> bool {
    let result = read_string_arg(db_path).and_then(api::db::init_db);
    match result {
        Ok(()) => {
            clear_last_error();
            true
        }
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn bayin_db_get_all_songs_json() -> *mut c_char {
    ok_string(
        api::db::db_get_all_songs()
            .and_then(|songs| serde_json::to_string(&songs).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_db_get_all_albums_json() -> *mut c_char {
    ok_string(
        api::db::db_get_all_albums()
            .and_then(|albums| serde_json::to_string(&albums).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_db_get_all_artists_json() -> *mut c_char {
    ok_string(
        api::db::db_get_all_artists()
            .and_then(|artists| serde_json::to_string(&artists).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_db_get_stream_servers_json() -> *mut c_char {
    ok_string(
        api::db::db_get_stream_servers()
            .and_then(|servers| serde_json::to_string(&servers).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_db_get_stream_playlists_json(server_id: *const c_char) -> *mut c_char {
    let result = read_string_arg(server_id)
        .and_then(api::db::db_get_stream_playlists)
        .and_then(|playlists| serde_json::to_string(&playlists).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_db_save_stream_server_json(config_json: *const c_char) -> *mut c_char {
    let result = read_string_arg(config_json)
        .and_then(|text| {
            serde_json::from_str::<StreamServerInput>(&text)
                .map_err(|err| format!("Invalid stream server config JSON: {err}"))
        })
        .and_then(api::db::db_save_stream_server);
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_db_delete_stream_server(server_id: *const c_char) -> bool {
    let result = read_string_arg(server_id).and_then(api::db::db_delete_stream_server);
    handle_bool_result(result)
}

#[no_mangle]
pub extern "C" fn bayin_stream_test_connection_json(config_json: *const c_char) -> *mut c_char {
    let result = read_string_arg(config_json)
        .and_then(|text| {
            serde_json::from_str::<StreamServerInput>(&text)
                .map_err(|err| format!("Invalid stream connection config JSON: {err}"))
        })
        .and_then(api::streaming::test_stream_connection)
        .and_then(|res| serde_json::to_string(&res).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_stream_sync_playlists_json(server_id: *const c_char) -> *mut c_char {
    let result = read_string_arg(server_id)
        .and_then(api::streaming::sync_stream_playlists)
        .and_then(|res| serde_json::to_string(&res).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_stream_get_playlist_songs_json(request_json: *const c_char) -> *mut c_char {
    let result = read_string_arg(request_json)
        .and_then(|text| {
            serde_json::from_str::<StreamPlaylistSongsRequest>(&text)
                .map_err(|err| format!("Invalid stream playlist request JSON: {err}"))
        })
        .and_then(api::streaming::get_stream_playlist_songs)
        .and_then(|songs| serde_json::to_string(&songs).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_get_music_metadata_json(file_path: *const c_char) -> *mut c_char {
    let result = read_string_arg(file_path)
        .and_then(api::scanner::get_music_metadata)
        .and_then(|song| serde_json::to_string(&song).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_get_lyrics_json(file_path: *const c_char) -> *mut c_char {
    let result = read_string_arg(file_path)
        .and_then(api::scanner::get_lyrics)
        .and_then(|lyrics| serde_json::to_string(&lyrics).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_list_directories_json(path: *const c_char) -> *mut c_char {
    let result = read_string_arg(path)
        .and_then(api::scanner::list_directories)
        .and_then(|entries| serde_json::to_string(&entries).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_scan_music_files_json(options_json: *const c_char) -> *mut c_char {
    let result = read_string_arg(options_json)
        .and_then(|text| {
            serde_json::from_str(&text).map_err(|err| format!("Invalid scan options JSON: {err}"))
        })
        .and_then(api::scanner::scan_music_files)
        .and_then(|songs| serde_json::to_string(&songs).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_scan_and_save_music_files_json(options_json: *const c_char) -> *mut c_char {
    let result = read_string_arg(options_json)
        .and_then(|text| {
            serde_json::from_str(&text).map_err(|err| format!("Invalid scan options JSON: {err}"))
        })
        .and_then(api::scanner::scan_and_save_music_files)
        .and_then(|summary| serde_json::to_string(&summary).map_err(|e| e.to_string()));
    ok_string(result)
}

#[no_mangle]
pub extern "C" fn bayin_backfill_song_covers_json() -> *mut c_char {
    ok_string(
        api::scanner::backfill_song_covers()
            .and_then(|summary| serde_json::to_string(&summary).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_db_clear_all_songs() -> bool {
    match api::db::db_clear_all_songs() {
        Ok(_) => {
            clear_last_error();
            true
        }
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn bayin_db_save_scan_config_json(config_json: *const c_char) -> bool {
    let result = read_string_arg(config_json)
        .and_then(|text| {
            serde_json::from_str(&text).map_err(|err| format!("Invalid scan config JSON: {err}"))
        })
        .and_then(api::db::db_save_scan_config);
    match result {
        Ok(()) => {
            clear_last_error();
            true
        }
        Err(err) => {
            set_last_error(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn bayin_db_get_scan_config_json() -> *mut c_char {
    ok_string(
        api::db::db_get_scan_config()
            .and_then(|maybe| serde_json::to_string(&maybe).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_start_file_watcher_json(directories_json: *const c_char) -> bool {
    let result = read_string_arg(directories_json)
        .and_then(|text| {
            serde_json::from_str::<Vec<String>>(&text)
                .map_err(|err| format!("Invalid watcher directories JSON: {err}"))
        })
        .and_then(api::watcher::start_file_watcher);
    handle_bool_result(result)
}

#[no_mangle]
pub extern "C" fn bayin_stop_file_watcher() -> bool {
    handle_bool_result(api::watcher::stop_file_watcher())
}

#[no_mangle]
pub extern "C" fn bayin_poll_file_watcher_events_json() -> *mut c_char {
    ok_string(
        api::watcher::poll_file_watcher_events(128)
            .and_then(|events| serde_json::to_string(&events).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_file_watcher_status_json() -> *mut c_char {
    ok_string(
        api::watcher::file_watcher_status()
            .and_then(|status| serde_json::to_string(&status).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_windows_taskbar_init() -> bool {
    handle_bool_result(api::taskbar_windows::init_windows_taskbar())
}

#[no_mangle]
pub extern "C" fn bayin_windows_taskbar_update_json(state_json: *const c_char) -> bool {
    let result = read_string_arg(state_json)
        .and_then(|text| {
            serde_json::from_str::<api::taskbar_windows::WindowsTaskbarState>(&text)
                .map_err(|err| format!("Invalid Windows taskbar state JSON: {err}"))
        })
        .and_then(api::taskbar_windows::update_windows_taskbar);
    handle_bool_result(result)
}

#[no_mangle]
pub extern "C" fn bayin_windows_taskbar_poll_events_json() -> *mut c_char {
    ok_string(
        api::taskbar_windows::poll_windows_taskbar_events(32)
            .and_then(|events| serde_json::to_string(&events).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_audio_play(source: *const c_char) -> bool {
    let result = read_string_arg(source).and_then(api::player::audio_play);
    handle_bool_result(result)
}

#[no_mangle]
pub extern "C" fn bayin_audio_pause() -> bool {
    handle_bool_result(api::player::audio_pause())
}

#[no_mangle]
pub extern "C" fn bayin_audio_resume() -> bool {
    handle_bool_result(api::player::audio_resume())
}

#[no_mangle]
pub extern "C" fn bayin_audio_stop() -> bool {
    handle_bool_result(api::player::audio_stop())
}

#[no_mangle]
pub extern "C" fn bayin_audio_seek(position_secs: f64) -> bool {
    handle_bool_result(api::player::audio_seek(position_secs))
}

#[no_mangle]
pub extern "C" fn bayin_audio_set_volume(volume: f64) -> bool {
    handle_bool_result(api::player::audio_set_volume(volume as f32))
}

#[no_mangle]
pub extern "C" fn bayin_audio_set_eq_enabled(enabled: bool) -> bool {
    handle_bool_result(api::player::audio_set_eq_enabled(enabled))
}

#[no_mangle]
pub extern "C" fn bayin_audio_set_eq_gains_json(gains_json: *const c_char) -> bool {
    let result = read_string_arg(gains_json)
        .and_then(|text| {
            let gains: Vec<f32> = serde_json::from_str(&text)
                .map_err(|err| format!("Invalid EQ gains JSON: {err}"))?;
            if gains.len() != 10 {
                return Err("EQ gains must contain exactly 10 items.".to_string());
            }
            let mut normalized = [0.0f32; 10];
            for (index, value) in gains.into_iter().enumerate() {
                normalized[index] = value.clamp(-12.0, 12.0);
            }
            Ok(normalized)
        })
        .and_then(api::player::audio_set_eq_gains);
    handle_bool_result(result)
}

#[no_mangle]
pub extern "C" fn bayin_audio_get_state_json() -> *mut c_char {
    ok_string(
        api::player::audio_get_state()
            .and_then(|state| serde_json::to_string(&state).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_audio_get_fft_json() -> *mut c_char {
    ok_string(
        api::player::audio_get_fft()
            .and_then(|fft| serde_json::to_string(&fft).map_err(|e| e.to_string())),
    )
}

#[no_mangle]
pub extern "C" fn bayin_audio_get_eq_state_json() -> *mut c_char {
    ok_string(
        api::player::audio_get_eq_state()
            .and_then(|eq| serde_json::to_string(&eq).map_err(|e| e.to_string())),
    )
}
