mod commands;
mod models;
mod utils;

use commands::{
    fetch_stream_songs, fetch_subsonic_songs, get_lyrics, get_music_metadata,
    get_stream_lyrics, get_stream_url, get_subsonic_lyrics, get_subsonic_stream_url,
    jellyfin_authenticate, list_directories, scan_music_files, test_stream_connection,
    test_subsonic_connection,
};
#[cfg(desktop)]
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init());

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
            get_subsonic_lyrics
        ])
        .setup(|_app| {
            // 桌面端：窗口状态已恢复，显示窗口
            #[cfg(desktop)]
            {
                if let Some(window) = _app.get_webview_window("main") {
                    let _ = window.as_ref().window().show();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
