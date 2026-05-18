fn main() {
    // macOS: link MediaPlayer framework for MPRemoteCommandCenter / MPNowPlayingInfoCenter
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=MediaPlayer");

    tauri_plugin::Builder::new(&[
        "initialize",
        "set_metadata",
        "set_playback_status",
        "set_position",
        "clear",
    ])
    .android_path("android")
    .build();
}
