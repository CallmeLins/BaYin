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
import android.webkit.WebSettings
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

    // ── 车机优化：前端资源全部内嵌在 APK 里，禁止磁盘 HTTP 缓存 ──────────
    // 否则升级 APK 后 WebView 仍可能命中旧缓存，出现"改了没生效"的假象；
    // 同时也能省掉一份缓存占用（车机内置存储紧张）。
    webView.settings.apply {
      cacheMode = WebSettings.LOAD_NO_CACHE
      domStorageEnabled = true
      javaScriptEnabled = true
      allowFileAccess = false
      allowContentAccess = false
      mediaPlaybackRequiresUserGesture = false
      setSupportMultipleWindows(false)
      builtInZoomControls = false
      displayZoomControls = false
    }

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

    /**
     * 枚举外接存储卷（U 盘 / SD 卡），返回 JSON 数组字符串。
     *
     * 实现说明：刻意不用 StorageVolume.getPath()/getDirectory() 这类隐藏或高版本 API
     * （Android 7~9 上反射受限、Android 10+ 才有 getDirectory），而是用
     * `getExternalFilesDirs(null)` 反推挂载根：每个返回路径形如
     *   /storage/XXXX-XXXX/Android/data/<pkg>/files
     * 截断掉 "/Android/" 之后即得到卷根，这种写法在 7.0~14 全版本可用。
     */
    @JavascriptInterface
    fun listStorageVolumes(): String {
      val out = StringBuilder("[")
      var first = true
      try {
        val dirs = getExternalFilesDirs(null)
        val seen = HashSet<String>()
        for (dir in dirs) {
          if (dir == null) continue
          val p = dir.absolutePath
          val cut = p.indexOf("/Android/")
          if (cut <= 0) continue
          val root = p.substring(0, cut)
          if (!seen.add(root)) continue

          val f = java.io.File(root)
          val readable = try {
            f.isDirectory && f.canRead() && (f.list()?.isNotEmpty() == true)
          } catch (e: Exception) {
            false
          }
          val st = android.os.StatFs(root)
          val freeMb = st.availableBlocksLong * st.blockSizeLong / (1024 * 1024)
          val totalMb = st.blockCountLong * st.blockSizeLong / (1024 * 1024)
          val primary = root.contains("/emulated/")
          val label = when {
            primary -> "内置存储"
            root.startsWith("/storage/") -> root.removePrefix("/storage/")
            else -> f.name
          }

          if (!first) out.append(',')
          first = false
          out.append("{")
          out.append("\"root\":").append(quote(root)).append(',')
          out.append("\"label\":").append(quote(label)).append(',')
          out.append("\"removable\":").append(!primary).append(',')
          out.append("\"readable\":").append(readable).append(',')
          out.append("\"freeMb\":").append(freeMb).append(',')
          out.append("\"totalMb\":").append(totalMb)
          out.append("}")
        }
      } catch (e: Exception) {
        Log.w(TAG, "listStorageVolumes failed", e)
      }
      out.append("]")
      val json = out.toString()
      Log.d(TAG, "listStorageVolumes => $json")
      return json
    }

    /** 申请「所有文件访问权限」（Android 11+ 读取 U 盘根目录需要） */
    @JavascriptInterface
    fun requestAllFilesAccess(): Boolean {
      if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return true
      if (android.os.Environment.isExternalStorageManager()) return true
      return try {
        val intent = Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION)
        intent.data = Uri.parse("package:$packageName")
        startActivity(intent)
        false
      } catch (e: Exception) {
        Log.w(TAG, "cannot open all-files access page", e)
        false
      }
    }

    @JavascriptInterface
    fun hasAllFilesAccess(): Boolean {
      return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        android.os.Environment.isExternalStorageManager()
      } else {
        ContextCompat.checkSelfPermission(
          this@MainActivity,
          android.Manifest.permission.READ_EXTERNAL_STORAGE
        ) == PackageManager.PERMISSION_GRANTED
      }
    }

    private fun quote(s: String): String {
      val sb = StringBuilder("\"")
      for (c in s) {
        when (c) {
          '"' -> sb.append("\\\"")
          '\\' -> sb.append("\\\\")
          '\n' -> sb.append("\\n")
          '\r' -> sb.append("\\r")
          else -> sb.append(c)
        }
      }
      return sb.append("\"").toString()
    }
  }
}
