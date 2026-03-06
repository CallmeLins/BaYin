package app.tauri.bayin.systemmedia

// Called by Android MediaSession/notification controls to control Rust playback directly.
object SystemMediaBridge {
    @JvmStatic external fun nativeGetNowPlayingJson(): String
    @JvmStatic external fun nativePlay()
    @JvmStatic external fun nativePause()
    @JvmStatic external fun nativeTogglePlayPause()
    @JvmStatic external fun nativeNext()
    @JvmStatic external fun nativePrevious()
    @JvmStatic external fun nativeSeekToMs(positionMs: Long)
}

