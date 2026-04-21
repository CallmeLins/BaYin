package app.tauri.bayin.systemmedia

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import android.media.AudioAttributes
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

class MediaPlaybackService : Service() {
    private lateinit var session: MediaSession
    private lateinit var audioManager: AudioManager
    private var focusRequest: AudioFocusRequest? = null
    private var shouldResumeAfterTransientLoss = false
    private var audioDeviceCallback: AudioDeviceCallback? = null

    private val scheduler = Executors.newSingleThreadScheduledExecutor()
    private var pollTask: ScheduledFuture<*>? = null

    private var isForeground = false
    private var lastNowPlayingId: String? = null
    private var lastArtworkPath: String? = null
    private var lastArtworkUrl: String? = null
    private var currentArtwork: android.graphics.Bitmap? = null

    override fun onCreate() {
        super.onCreate()

        audioManager = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        session = MediaSession(this, "BaYin")
        session.setFlags(
            MediaSession.FLAG_HANDLES_MEDIA_BUTTONS or
                MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS
        )
        session.setCallback(object : MediaSession.Callback() {
            override fun onPlay() {
                tryRequestAudioFocus()
                safeNative { SystemMediaBridge.nativePlay() }
                updateFromRust()
            }

            override fun onPause() {
                safeNative { SystemMediaBridge.nativePause() }
                updateFromRust()
            }

            override fun onSkipToNext() {
                safeNative { SystemMediaBridge.nativeNext() }
                updateFromRust()
            }

            override fun onSkipToPrevious() {
                safeNative { SystemMediaBridge.nativePrevious() }
                updateFromRust()
            }

            override fun onSeekTo(pos: Long) {
                safeNative { SystemMediaBridge.nativeSeekToMs(pos) }
                updateFromRust()
            }
        })
        session.isActive = true

        registerAudioDeviceCallback()

        ensureNotificationChannel()
        // Delay polling until the Rust side has finished managing app state.
        scheduler.schedule({ startPolling() }, 800, TimeUnit.MILLISECONDS)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_PLAY_PAUSE -> {
                tryRequestAudioFocus()
                safeNative { SystemMediaBridge.nativeTogglePlayPause() }
            }
            ACTION_NEXT -> safeNative { SystemMediaBridge.nativeNext() }
            ACTION_PREVIOUS -> safeNative { SystemMediaBridge.nativePrevious() }
        }

        updateFromRust()
        return START_STICKY
    }

    override fun onDestroy() {
        pollTask?.cancel(true)
        scheduler.shutdownNow()
        unregisterAudioDeviceCallback()
        session.release()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun startPolling() {
        pollTask?.cancel(true)
        pollTask = scheduler.scheduleAtFixedRate(
            { updateFromRust() },
            0,
            1,
            TimeUnit.SECONDS
        )
    }

    private fun updateFromRust() {
        val json = runCatching { SystemMediaBridge.nativeGetNowPlayingJson() }.getOrNull() ?: "{}"
        val obj = runCatching { JSONObject(json) }.getOrNull() ?: JSONObject()

        val id = obj.optString("id", "")
        val title = obj.optString("title", "")
        val artist = obj.optString("artist", "")
        val album = obj.optString("album", "")
        val artworkPath = obj.optString("artwork_path", "")
        val artworkUrl = obj.optString("artwork_url", "")
        val isPlaying = obj.optBoolean("is_playing", false)
        val positionMs = obj.optLong("position_ms", 0L).coerceAtLeast(0L)
        val durationMs = obj.optLong("duration_ms", 0L).coerceAtLeast(0L)
        val canNext = obj.optBoolean("can_next", false)
        val canPrevious = obj.optBoolean("can_previous", false)

        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (id.isEmpty()) {
            lastNowPlayingId = null
            // Nothing selected yet; keep the MediaSession alive but do not show a notification.
            val playbackState = PlaybackState.Builder()
                .setActions(0)
                .setState(PlaybackState.STATE_NONE, 0L, 0.0f, SystemClock.elapsedRealtime())
                .build()
            session.setPlaybackState(playbackState)

            if (isForeground) {
                stopForeground(true)
                isForeground = false
            }
            nm.cancel(NOTIFICATION_ID)
            return
        }

        // Update metadata when:
        // - track changes, OR
        // - artwork becomes available/changes (e.g. stream cover gets cached after first poll)
        val isTrackChanged = id.isNotEmpty() && id != lastNowPlayingId
        val isArtworkChanged =
            (artworkPath.isNotEmpty() && artworkPath != lastArtworkPath) ||
            (artworkPath.isEmpty() && artworkUrl.isNotEmpty() && artworkUrl != lastArtworkUrl)

        if (id.isNotEmpty() && (isTrackChanged || isArtworkChanged)) {
            if (isTrackChanged) {
                lastNowPlayingId = id
                currentArtwork = null
                lastArtworkPath = null
                lastArtworkUrl = null
            }

            val metaBuilder = MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, artist)
                .putString(MediaMetadata.METADATA_KEY_ALBUM, album)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, durationMs)

            // Attach album art if available (this is what system UIs use for lockscreen / cast / flows).
            val art = if (artworkPath.isNotEmpty()) {
                if (artworkPath == lastArtworkPath && currentArtwork != null) {
                    currentArtwork
                } else {
                    lastArtworkPath = artworkPath
                    lastArtworkUrl = null
                    runCatching { BitmapFactory.decodeFile(artworkPath) }.getOrNull()
                }
            } else if (artworkUrl.isNotEmpty()) {
                if (artworkUrl == lastArtworkUrl && currentArtwork != null) {
                    currentArtwork
                } else {
                    lastArtworkUrl = artworkUrl
                    lastArtworkPath = null
                    runCatching {
                        // URL may include auth query params; this runs on a background thread (poll executor).
                        val conn = (URL(artworkUrl).openConnection() as HttpURLConnection).apply {
                            connectTimeout = 5_000
                            readTimeout = 10_000
                            instanceFollowRedirects = true
                        }
                        conn.inputStream.use { input -> BitmapFactory.decodeStream(input) }
                    }.getOrNull()
                }
            } else {
                lastArtworkPath = null
                lastArtworkUrl = null
                null
            }
            if (art != null) {
                currentArtwork = art
                metaBuilder
                    .putBitmap(MediaMetadata.METADATA_KEY_ART, art)
                    .putBitmap(MediaMetadata.METADATA_KEY_ALBUM_ART, art)
            }

            session.setMetadata(metaBuilder.build())
        }

        val actions = buildActions(canPrevious, canNext, durationMs > 0L)
        val state = if (isPlaying) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED
        val playbackState = PlaybackState.Builder()
            .setActions(actions)
            .setState(state, positionMs, if (isPlaying) 1.0f else 0.0f, SystemClock.elapsedRealtime())
            .build()
        session.setPlaybackState(playbackState)

        // Notification/foreground policy.
        val notif = buildNotification(isPlaying, canPrevious, canNext, title, artist)
        if (isPlaying) {
            if (!isForeground) {
                startForeground(NOTIFICATION_ID, notif)
                isForeground = true
            } else {
                nm.notify(NOTIFICATION_ID, notif)
            }
        } else {
            if (isForeground) {
                stopForeground(false)
                isForeground = false
            }
            nm.notify(NOTIFICATION_ID, notif)
        }
    }

    private fun buildActions(canPrevious: Boolean, canNext: Boolean, canSeek: Boolean): Long {
        var actions = PlaybackState.ACTION_PLAY_PAUSE or
            PlaybackState.ACTION_PLAY or
            PlaybackState.ACTION_PAUSE
        if (canPrevious) actions = actions or PlaybackState.ACTION_SKIP_TO_PREVIOUS
        if (canNext) actions = actions or PlaybackState.ACTION_SKIP_TO_NEXT
        if (canSeek) actions = actions or PlaybackState.ACTION_SEEK_TO
        return actions
    }

    @Suppress("DEPRECATION")
    private fun buildNotification(
        isPlaying: Boolean,
        canPrevious: Boolean,
        canNext: Boolean,
        title: String,
        artist: String
    ): Notification {
        val playPauseIntent = PendingIntent.getService(
            this,
            1,
            Intent(this, MediaPlaybackService::class.java).setAction(ACTION_PLAY_PAUSE),
            pendingIntentFlags()
        )
        val nextIntent = PendingIntent.getService(
            this,
            2,
            Intent(this, MediaPlaybackService::class.java).setAction(ACTION_NEXT),
            pendingIntentFlags()
        )
        val prevIntent = PendingIntent.getService(
            this,
            3,
            Intent(this, MediaPlaybackService::class.java).setAction(ACTION_PREVIOUS),
            pendingIntentFlags()
        )

        val builder = if (Build.VERSION.SDK_INT >= 26) {
            Notification.Builder(this, NOTIFICATION_CHANNEL_ID)
        } else {
            Notification.Builder(this)
        }

        val appIntent = packageManager.getLaunchIntentForPackage(packageName)
        val contentIntent = if (appIntent != null) {
            PendingIntent.getActivity(this, 0, appIntent, pendingIntentFlags())
        } else null

        builder.setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(if (title.isNotEmpty()) title else "BaYin")
            .setContentText(artist.ifEmpty { " " })
            .apply {
                if (contentIntent != null) setContentIntent(contentIntent)
            }
            .setOngoing(isPlaying)
            .setOnlyAlertOnce(true)
            .apply {
                // Large icon is shown in notification shade, and some system panels pick it up too.
                currentArtwork?.let { setLargeIcon(it) }
            }

        val compact = mutableListOf<Int>()
        var idx = 0
        if (canPrevious) {
            builder.addAction(android.R.drawable.ic_media_previous, "Previous", prevIntent)
            compact.add(idx)
            idx += 1
        }
        builder.addAction(
            if (isPlaying) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play,
            if (isPlaying) "Pause" else "Play",
            playPauseIntent
        )
        compact.add(idx)
        idx += 1
        if (canNext) {
            builder.addAction(android.R.drawable.ic_media_next, "Next", nextIntent)
            compact.add(idx)
            idx += 1
        }

        val style = Notification.MediaStyle()
            .setMediaSession(session.sessionToken)
            .apply {
                if (compact.isNotEmpty()) {
                    setShowActionsInCompactView(*compact.toIntArray())
                }
            }
        builder.setStyle(style)

        return builder.build()
    }

    private fun pendingIntentFlags(): Int {
        var flags = PendingIntent.FLAG_UPDATE_CURRENT
        if (Build.VERSION.SDK_INT >= 23) {
            flags = flags or PendingIntent.FLAG_IMMUTABLE
        }
        return flags
    }

    private fun ensureNotificationChannel() {
        if (Build.VERSION.SDK_INT < 26) return
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            NOTIFICATION_CHANNEL_ID,
            "Media playback",
            NotificationManager.IMPORTANCE_LOW
        )
        nm.createNotificationChannel(channel)
    }

    private fun tryRequestAudioFocus() {
        // AudioFocus is important on HyperOS/MIUI to get correct routing + system controls behavior.
        if (Build.VERSION.SDK_INT >= 26) {
            if (focusRequest == null) {
                focusRequest = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_MEDIA)
                            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                            .build()
                    )
                    .setOnAudioFocusChangeListener { change ->
                        when (change) {
                            AudioManager.AUDIOFOCUS_LOSS -> {
                                shouldResumeAfterTransientLoss = false
                                safeNative { SystemMediaBridge.nativePause() }
                            }
                            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                                shouldResumeAfterTransientLoss = isPlaybackActive()
                                safeNative { SystemMediaBridge.nativePause() }
                            }
                            AudioManager.AUDIOFOCUS_GAIN -> {
                                if (shouldResumeAfterTransientLoss) {
                                    shouldResumeAfterTransientLoss = false
                                    safeNative { SystemMediaBridge.nativeRefreshOutput() }
                                    safeNative { SystemMediaBridge.nativeResume() }
                                    updateFromRust()
                                }
                            }
                        }
                    }
                    .build()
            }
            audioManager.requestAudioFocus(focusRequest!!)
        } else {
            @Suppress("DEPRECATION")
            audioManager.requestAudioFocus(
                { change ->
                    when (change) {
                        AudioManager.AUDIOFOCUS_LOSS -> {
                            shouldResumeAfterTransientLoss = false
                            safeNative { SystemMediaBridge.nativePause() }
                        }
                        AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                            shouldResumeAfterTransientLoss = isPlaybackActive()
                            safeNative { SystemMediaBridge.nativePause() }
                        }
                        AudioManager.AUDIOFOCUS_GAIN -> {
                            if (shouldResumeAfterTransientLoss) {
                                shouldResumeAfterTransientLoss = false
                                safeNative { SystemMediaBridge.nativeRefreshOutput() }
                                safeNative { SystemMediaBridge.nativeResume() }
                                updateFromRust()
                            }
                        }
                    }
                },
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN
            )
        }
    }

    private fun registerAudioDeviceCallback() {
        if (Build.VERSION.SDK_INT < 23 || audioDeviceCallback != null) return
        audioDeviceCallback = object : AudioDeviceCallback() {
            override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>) {
                handleAudioRouteChange(addedDevices)
            }

            override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>) {
                handleAudioRouteChange(removedDevices)
            }
        }
        audioManager.registerAudioDeviceCallback(audioDeviceCallback, null)
    }

    private fun unregisterAudioDeviceCallback() {
        if (Build.VERSION.SDK_INT < 23) return
        audioDeviceCallback?.let { audioManager.unregisterAudioDeviceCallback(it) }
        audioDeviceCallback = null
    }

    private fun handleAudioRouteChange(devices: Array<out AudioDeviceInfo>) {
        if (!devices.any { it.isRelevantOutputRoute() }) return
        if (!isPlaybackActive()) return

        scheduler.execute {
            tryRequestAudioFocus()
            safeNative { SystemMediaBridge.nativeRefreshOutput() }
            updateFromRust()
        }
    }

    private fun AudioDeviceInfo.isRelevantOutputRoute(): Boolean =
        isSink && when (type) {
            AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
            AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
            AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
            AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
            AudioDeviceInfo.TYPE_BLE_HEADSET,
            AudioDeviceInfo.TYPE_BLE_SPEAKER,
            AudioDeviceInfo.TYPE_BLE_BROADCAST,
            AudioDeviceInfo.TYPE_WIRED_HEADSET,
            AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
            AudioDeviceInfo.TYPE_USB_HEADSET,
            AudioDeviceInfo.TYPE_USB_DEVICE,
            AudioDeviceInfo.TYPE_USB_ACCESSORY,
            AudioDeviceInfo.TYPE_DOCK,
            AudioDeviceInfo.TYPE_HEARING_AID,
            AudioDeviceInfo.TYPE_REMOTE_SUBMIX,
            AudioDeviceInfo.TYPE_LINE_ANALOG,
            AudioDeviceInfo.TYPE_LINE_DIGITAL,
            AudioDeviceInfo.TYPE_HDMI,
            AudioDeviceInfo.TYPE_HDMI_ARC,
            AudioDeviceInfo.TYPE_HDMI_EARC -> true
            else -> false
        }

    private fun isPlaybackActive(): Boolean {
        val state = session.controller.playbackState?.state ?: PlaybackState.STATE_NONE
        return state == PlaybackState.STATE_PLAYING || state == PlaybackState.STATE_BUFFERING
    }

    private fun safeNative(block: () -> Unit) {
        runCatching { block() }
    }

    companion object {
        private const val NOTIFICATION_CHANNEL_ID = "bayin_media_playback"
        private const val NOTIFICATION_ID = 0xBA71 // constant but recognizable

        private const val ACTION_PLAY_PAUSE = "app.tauri.bayin.systemmedia.action.PLAY_PAUSE"
        private const val ACTION_NEXT = "app.tauri.bayin.systemmedia.action.NEXT"
        private const val ACTION_PREVIOUS = "app.tauri.bayin.systemmedia.action.PREVIOUS"

        fun ensureRunning(ctx: Context) {
            val intent = Intent(ctx, MediaPlaybackService::class.java)
            ctx.startService(intent)
        }

        fun stop(ctx: Context) {
            ctx.stopService(Intent(ctx, MediaPlaybackService::class.java))
        }
    }
}
