package com.hao.bayin

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.graphics.Color
import android.view.View
import android.view.WindowInsetsController
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen

class MainActivity : TauriActivity() {
  private val TAG = "BaYinMainActivity"
  private val PERMISSION_REQUEST_CODE = 1001
  private var webViewRef: WebView? = null

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

    super.onCreate(savedInstanceState)
    Log.d(TAG, "onCreate called")
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webViewRef = webView
    Log.d(TAG, "onWebViewCreate called, adding AndroidBridge")

    // 设置 WebView 背景透明，避免底部手势导航区域透出白色背景
    webView.setBackgroundColor(Color.TRANSPARENT)

    // 添加 JavaScript 接口
    webView.addJavascriptInterface(AndroidBridge(), "AndroidBridge")
  }

  /**
   * 设置状态栏和导航栏图标颜色
   * @param isDark true = 深色主题（白色图标），false = 浅色主题（黑色图标）
   */
  private fun setStatusBarIcons(isDark: Boolean) {
    runOnUiThread {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        // Android 11+
        val controller = window.insetsController
        if (isDark) {
          // 深色主题：状态栏和导航栏指示条为白色
          controller?.setSystemBarsAppearance(
            0,
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
              WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
          )
        } else {
          // 浅色主题：状态栏和导航栏指示条为黑色
          controller?.setSystemBarsAppearance(
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
              WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
              WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
          )
        }
      } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        // Android 6-10
        @Suppress("DEPRECATION")
        val lightFlags = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or
          View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
        if (isDark) {
          window.decorView.systemUiVisibility =
            window.decorView.systemUiVisibility and lightFlags.inv()
        } else {
          window.decorView.systemUiVisibility =
            window.decorView.systemUiVisibility or lightFlags
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
  }
}
