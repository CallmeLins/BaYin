package app.tauri.bayin.systemmedia

import android.util.Log

// Called by Android MediaSession/notification controls to control Rust playback directly.
object SystemMediaBridge {
    private const val TAG = "BaYinSystemMedia"

    init {
        // The ForegroundService might be started by the system without the UI Activity.
        // Ensure the Rust shared library is loaded so JNI symbols are available.
        val loadResult = runCatching { System.loadLibrary("bayin_lib") }
        loadResult.onFailure {
            Log.e(TAG, "System.loadLibrary(bayin_lib) failed: ${it.message}", it)
        }
        // Probe one JNI symbol so an unresolved-link error surfaces in logcat
        // instead of being silently swallowed by every poll.
        val probe = runCatching { nativeGetNowPlayingJson() }
        probe.onSuccess { Log.i(TAG, "JNI bridge ready, initial state=$it") }
        probe.onFailure { Log.e(TAG, "JNI bridge probe failed: ${it.message}", it) }
    }

    @JvmStatic external fun nativeGetNowPlayingJson(): String
    @JvmStatic external fun nativePlay()
    @JvmStatic external fun nativePause()
    @JvmStatic external fun nativeResume()
    @JvmStatic external fun nativeTogglePlayPause()
    @JvmStatic external fun nativeNext()
    @JvmStatic external fun nativePrevious()
    @JvmStatic external fun nativeSeekToMs(positionMs: Long)
    @JvmStatic external fun nativeRefreshOutput()

    /**
     * Register a Runnable that Rust will invoke whenever the now-playing
     * state changes. Pass `null` to clear. Used by the Service to refresh
     * the MediaSession immediately on pause/seek instead of waiting for the
     * next safety poll.
     */
    @JvmStatic external fun nativeSetUpdateCallback(callback: Runnable?)
}
