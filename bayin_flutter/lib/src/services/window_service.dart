import 'dart:io';
import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:window_manager/window_manager.dart';

/// Desktop window chrome setup (Windows / Linux / macOS).
///
/// Phase 2 scaffolds the frameless-plus-custom-titlebar arrangement:
///   * Windows / Linux: titleBarStyle.hidden, custom Dart titlebar paints the
///     three window buttons.
///   * macOS: titleBarStyle.hidden + titleBarHeight makes the native
///     traffic-lights render over an overlay titlebar area. Phase 8 polishes
///     the traffic-light placement via `macos_ui`.
///
/// On mobile / web this is a no-op.
class WindowService {
  const WindowService._();

  static Future<void> init() async {
    if (kIsWeb) return;
    if (!(Platform.isWindows || Platform.isLinux || Platform.isMacOS)) return;

    await windowManager.ensureInitialized();

    const windowOptions = WindowOptions(
      size: Size(1200, 780),
      minimumSize: Size(960, 620),
      center: true,
      backgroundColor: Color(0x00000000),
      skipTaskbar: false,
      titleBarStyle: TitleBarStyle.hidden,
      windowButtonVisibility: false,
      title: 'BaYin',
    );

    await windowManager.waitUntilReadyToShow(windowOptions, () async {
      await windowManager.show();
      await windowManager.focus();
    });
  }
}
