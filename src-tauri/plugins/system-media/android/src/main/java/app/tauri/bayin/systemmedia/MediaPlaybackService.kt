package app.tauri.bayin.systemmedia

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import org.json.JSONObject
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

class MediaPlaybackService : Service() {
    private lateinit var session: MediaSession
    private lateinit var audioManager: AudioManager
    private var focusRequest: AudioFocusRequest? = null

    private val scheduler = Executors.newSingleThreadScheduledExecutor()
    private var pollTask: ScheduledFuture<*>? = null

    private var isForeground = false
    private var lastNowPlayingId: String? = null

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

        ensureNotificationChannel()
        startPolling()
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

        // Metadata only needs updating when the track changes.
        if (id.isNotEmpty() && id != lastNowPlayingId) {
            lastNowPlayingId = id
            val meta = MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, artist)
                .putString(MediaMetadata.METADATA_KEY_ALBUM, album)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, durationMs)
                .build()
            session.setMetadata(meta)
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
                        if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
                            safeNative { SystemMediaBridge.nativePause() }
                        }
                    }
                    .build()
            }
            audioManager.requestAudioFocus(focusRequest!!)
        } else {
            @Suppress("DEPRECATION")
            audioManager.requestAudioFocus(
                { change ->
                    if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
                        safeNative { SystemMediaBridge.nativePause() }
                    }
                },
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN
            )
        }
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
