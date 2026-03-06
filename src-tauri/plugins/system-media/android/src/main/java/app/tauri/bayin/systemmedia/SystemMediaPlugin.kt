package app.tauri.bayin.systemmedia

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@TauriPlugin
class SystemMediaPlugin(private val activity: Activity) : Plugin(activity) {
    init {
        // Keep the service available; it will only go foreground when playback is active.
        MediaPlaybackService.ensureRunning(activity.applicationContext)
    }

    @Command
    fun ensureService(invoke: Invoke) {
        try {
            MediaPlaybackService.ensureRunning(activity.applicationContext)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }

    @Command
    fun stopService(invoke: Invoke) {
        try {
            MediaPlaybackService.stop(activity.applicationContext)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }
}

