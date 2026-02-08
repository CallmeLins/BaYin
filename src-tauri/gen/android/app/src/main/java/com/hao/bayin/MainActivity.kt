package com.hao.bayin

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.media.MediaPlayer
import android.net.Uri
import android.os.Bundle
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.View
import android.view.WindowInsetsController
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import java.io.File

class MainActivity : TauriActivity() {
  private val TAG = "BaYinMainActivity"
  private val PERMISSION_REQUEST_CODE = 1001
  private var webViewRef: WebView? = null

  // MediaPlayer 相关
  private var mediaPlayer: MediaPlayer? = null
  private val handler = Handler(Looper.getMainLooper())
  private var progressRunnable: Runnable? = null
  private var currentFilePath: String? = null

  // Splash screen 相关
  private var keepSplashScreen = true
  private val splashMinDuration = 1000L // 最小显示 1 秒

  override fun onCreate(savedInstanceState: Bundle?) {
    // 安装 Splash Screen，必须在 super.onCreate 之前调用
    val splashScreen = installSplashScreen()

    // 记录启动时间
    val startTime = System.currentTimeMillis()

    // 设置保持条件：至少显示 1 秒
    splashScreen.setKeepOnScreenCondition {
      val elapsed = System.currentTimeMillis() - startTime
      elapsed < splashMinDuration
    }

    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    Log.d(TAG, "onCreate called")

    // 默认设置为深色模式的状态栏图标（白色）
    setStatusBarIcons(true)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webViewRef = webView
    Log.d(TAG, "onWebViewCreate called, adding AndroidBridge")

    // 添加 JavaScript 接口
    webView.addJavascriptInterface(AndroidBridge(), "AndroidBridge")
  }

  override fun onDestroy() {
    super.onDestroy()
    releaseMediaPlayer()
  }

  private fun releaseMediaPlayer() {
    progressRunnable?.let { handler.removeCallbacks(it) }
    progressRunnable = null
    mediaPlayer?.release()
    mediaPlayer = null
    currentFilePath = null
  }

  /**
   * 设置状态栏图标颜色
   * @param isDark true = 深色主题（白色图标），false = 浅色主题（黑色图标）
   */
  private fun setStatusBarIcons(isDark: Boolean) {
    runOnUiThread {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        // Android 11+
        if (isDark) {
          // 深色主题：状态栏图标为白色
          window.insetsController?.setSystemBarsAppearance(
            0,
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
          )
        } else {
          // 浅色主题：状态栏图标为黑色
          window.insetsController?.setSystemBarsAppearance(
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
          )
        }
      } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        // Android 6-10
        @Suppress("DEPRECATION")
        if (isDark) {
          window.decorView.systemUiVisibility =
            window.decorView.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
        } else {
          window.decorView.systemUiVisibility =
            window.decorView.systemUiVisibility or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
        }
      }
    }
  }

  /**
   * 获取需要请求的权限
   */
  private fun getRequiredPermission(): String {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      Manifest.permission.READ_MEDIA_AUDIO
    } else {
      Manifest.permission.READ_EXTERNAL_STORAGE
    }
  }

  /**
   * 请求存储权限
   */
  private fun requestStoragePermission() {
    val permission = getRequiredPermission()
    Log.d(TAG, "requestStoragePermission called, permission: $permission")

    // 检查是否已有权限
    if (ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED) {
      Log.d(TAG, "Permission already granted")
      notifyPermissionResult(true)
      return
    }

    // 检查是否应该显示权限说明（用户之前拒绝过但没选"不再询问"）
    val shouldShowRationale = ActivityCompat.shouldShowRequestPermissionRationale(this, permission)
    Log.d(TAG, "shouldShowRequestPermissionRationale: $shouldShowRationale")

    if (!shouldShowRationale && ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_DENIED) {
      // 可能是首次请求，也可能是用户选择了"不再询问"
      // 我们先尝试请求，如果系统不弹窗，用户需要手动去设置
      Log.d(TAG, "Attempting to request permission")
    }

    // 请求权限
    ActivityCompat.requestPermissions(this, arrayOf(permission), PERMISSION_REQUEST_CODE)
    Log.d(TAG, "requestPermissions called")
  }

  /**
   * 打开应用设置页面
   */
  private fun openAppSettings() {
    val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)
    intent.data = Uri.fromParts("package", packageName, null)
    startActivity(intent)
  }

  /**
   * 通知 WebView 权限结果
   */
  private fun notifyPermissionResult(granted: Boolean) {
    Log.d(TAG, "notifyPermissionResult: $granted")
    webViewRef?.post {
      webViewRef?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('android-permission-result', { detail: { granted: $granted } }))",
        null
      )
    }
  }

  /**
   * 通知 WebView 播放进度
   */
  private fun notifyProgress(currentTime: Double, duration: Double) {
    webViewRef?.post {
      webViewRef?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('android-audio-progress', { detail: { currentTime: $currentTime, duration: $duration } }))",
        null
      )
    }
  }

  /**
   * 通知 WebView 播放结束
   */
  private fun notifyEnded() {
    webViewRef?.post {
      webViewRef?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('android-audio-ended'))",
        null
      )
    }
  }

  /**
   * 通知 WebView 播放错误
   */
  private fun notifyError(message: String) {
    webViewRef?.post {
      val escapedMessage = message.replace("'", "\\'")
      webViewRef?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('android-audio-error', { detail: { message: '$escapedMessage' } }))",
        null
      )
    }
  }

  /**
   * 开始进度更新
   */
  private fun startProgressUpdates() {
    progressRunnable?.let { handler.removeCallbacks(it) }
    progressRunnable = object : Runnable {
      override fun run() {
        mediaPlayer?.let { mp ->
          if (mp.isPlaying) {
            val currentTime = mp.currentPosition / 1000.0
            val duration = mp.duration / 1000.0
            notifyProgress(currentTime, duration)
          }
        }
        handler.postDelayed(this, 250) // 每 250ms 更新一次
      }
    }
    handler.post(progressRunnable!!)
  }

  /**
   * 停止进度更新
   */
  private fun stopProgressUpdates() {
    progressRunnable?.let { handler.removeCallbacks(it) }
  }

  override fun onRequestPermissionsResult(
    requestCode: Int,
    permissions: Array<out String>,
    grantResults: IntArray
  ) {
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    Log.d(TAG, "onRequestPermissionsResult: requestCode=$requestCode, results=${grantResults.contentToString()}")

    if (requestCode == PERMISSION_REQUEST_CODE) {
      val granted = grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED
      notifyPermissionResult(granted)
    }
  }

  /**
   * JavaScript 接口，供前端调用
   */
  inner class AndroidBridge {
    @JavascriptInterface
    fun setTheme(isDark: Boolean) {
      Log.d(TAG, "AndroidBridge.setTheme called: isDark=$isDark")
      setStatusBarIcons(isDark)
    }

    @JavascriptInterface
    fun requestPermission() {
      Log.d(TAG, "AndroidBridge.requestPermission called")
      runOnUiThread {
        requestStoragePermission()
      }
    }

    @JavascriptInterface
    fun checkPermission(): Boolean {
      val permission = getRequiredPermission()
      val granted = ContextCompat.checkSelfPermission(this@MainActivity, permission) == PackageManager.PERMISSION_GRANTED
      Log.d(TAG, "AndroidBridge.checkPermission: $granted")
      return granted
    }

    @JavascriptInterface
    fun openSettings() {
      Log.d(TAG, "AndroidBridge.openSettings called")
      runOnUiThread {
        openAppSettings()
      }
    }

    // ========== 音频播放接口 ==========

    @JavascriptInterface
    fun playAudio(filePath: String) {
      Log.d(TAG, "AndroidBridge.playAudio: $filePath")
      runOnUiThread {
        try {
          // 如果是同一个文件且已经有 MediaPlayer，直接播放
          if (currentFilePath == filePath && mediaPlayer != null) {
            mediaPlayer?.start()
            startProgressUpdates()
            return@runOnUiThread
          }

          // 释放旧的 MediaPlayer
          releaseMediaPlayer()

          // 创建新的 MediaPlayer
          val file = File(filePath)
          if (!file.exists()) {
            notifyError("File not found: $filePath")
            return@runOnUiThread
          }

          mediaPlayer = MediaPlayer().apply {
            setDataSource(filePath)
            setOnPreparedListener { mp ->
              mp.start()
              startProgressUpdates()
              val duration = mp.duration / 1000.0
              notifyProgress(0.0, duration)
            }
            setOnCompletionListener {
              stopProgressUpdates()
              notifyEnded()
            }
            setOnErrorListener { _, what, extra ->
              Log.e(TAG, "MediaPlayer error: what=$what, extra=$extra")
              stopProgressUpdates()
              notifyError("Playback error: $what")
              true
            }
            prepareAsync()
          }
          currentFilePath = filePath
        } catch (e: Exception) {
          Log.e(TAG, "playAudio error", e)
          notifyError(e.message ?: "Unknown error")
        }
      }
    }

    @JavascriptInterface
    fun pauseAudio() {
      Log.d(TAG, "AndroidBridge.pauseAudio")
      runOnUiThread {
        mediaPlayer?.pause()
        stopProgressUpdates()
      }
    }

    @JavascriptInterface
    fun resumeAudio() {
      Log.d(TAG, "AndroidBridge.resumeAudio")
      runOnUiThread {
        mediaPlayer?.start()
        startProgressUpdates()
      }
    }

    @JavascriptInterface
    fun stopAudio() {
      Log.d(TAG, "AndroidBridge.stopAudio")
      runOnUiThread {
        releaseMediaPlayer()
      }
    }

    @JavascriptInterface
    fun seekAudio(positionSeconds: Double) {
      Log.d(TAG, "AndroidBridge.seekAudio: $positionSeconds")
      runOnUiThread {
        mediaPlayer?.seekTo((positionSeconds * 1000).toInt())
      }
    }

    @JavascriptInterface
    fun setVolume(volume: Float) {
      Log.d(TAG, "AndroidBridge.setVolume: $volume")
      runOnUiThread {
        mediaPlayer?.setVolume(volume, volume)
      }
    }

    @JavascriptInterface
    fun isPlaying(): Boolean {
      return mediaPlayer?.isPlaying ?: false
    }

    @JavascriptInterface
    fun getCurrentPosition(): Double {
      return (mediaPlayer?.currentPosition ?: 0) / 1000.0
    }

    @JavascriptInterface
    fun getDuration(): Double {
      return (mediaPlayer?.duration ?: 0) / 1000.0
    }
  }
}
