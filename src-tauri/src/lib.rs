mod commands;
mod models;
mod utils;

use commands::{
    fetch_navidrome_songs, get_lyrics, get_music_metadata, get_navidrome_lyrics,
    get_navidrome_stream_url, list_directories, scan_music_files,
    test_navidrome_connection,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init());

    // 窗口状态插件仅桌面端使用
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            scan_music_files,
            get_music_metadata,
            get_lyrics,
            list_directories,
            test_navidrome_connection,
            fetch_navidrome_songs,
            get_navidrome_stream_url,
            get_navidrome_lyrics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
