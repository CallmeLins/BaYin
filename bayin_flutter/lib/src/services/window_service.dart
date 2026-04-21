import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:window_manager/window_manager.dart';
import 'package:macos_ui/macos_ui.dart';

/// Desktop window chrome + state persistence (Windows / Linux / macOS).
///
/// On mobile / web this is a no-op.
class WindowService with WindowListener {
  WindowService._();

  static final WindowService _instance = WindowService._();

  static Future<void> init() => _instance._init();

  bool _initialized = false;
  bool _listenerAttached = false;
  Timer? _persistDebounce;

  Future<void> _init() async {
    if (_initialized) {
      return;
    }
    if (kIsWeb) return;
    if (!_isDesktop) return;
    if (Platform.isMacOS) {
      await _configureMacosWindow();
    }

    await windowManager.ensureInitialized();

    final restored = await _loadState();

    final windowOptions = WindowOptions(
      size: restored?.size ?? const Size(1200, 780),
      minimumSize: const Size(960, 620),
      center: restored == null,
      backgroundColor: const Color(0x00000000),
      skipTaskbar: false,
      titleBarStyle: TitleBarStyle.hidden,
      windowButtonVisibility: false,
      title: 'BaYin',
    );

    await windowManager.waitUntilReadyToShow(windowOptions, () async {
      if (restored != null) {
        await windowManager.setBounds(restored.bounds);
      }
      await windowManager.show();
      await windowManager.focus();
      if (restored?.isMaximized ?? false) {
        await windowManager.maximize();
      }
    });

    if (!_listenerAttached) {
      windowManager.addListener(this);
      _listenerAttached = true;
    }
    _initialized = true;
    await _persistNow();
  }

  bool get _isDesktop =>
      Platform.isWindows || Platform.isLinux || Platform.isMacOS;

  Future<void> _configureMacosWindow() async {
    try {
      const config = MacosWindowUtilsConfig();
      await config.apply();
    } catch (_) {
      // Best-effort only: keep boot resilient if macOS window utils fail.
    }
  }

  @override
  void onWindowMove() => _schedulePersist();

  @override
  void onWindowResize() => _schedulePersist();

  @override
  void onWindowMaximize() => _schedulePersist();

  @override
  void onWindowUnmaximize() => _schedulePersist();

  void _schedulePersist() {
    _persistDebounce?.cancel();
    _persistDebounce = Timer(
      const Duration(milliseconds: 350),
      () => unawaited(_persistNow()),
    );
  }

  Future<void> _persistNow() async {
    if (kIsWeb || !_isDesktop) {
      return;
    }
    try {
      final bounds = await windowManager.getBounds();
      final isMaximized = await windowManager.isMaximized();
      final state = _WindowState(
        x: bounds.left,
        y: bounds.top,
        width: bounds.width,
        height: bounds.height,
        isMaximized: isMaximized,
      );
      await _writeState(state);
    } catch (_) {
      // Ignore transient window-manager errors and keep app responsive.
    }
  }

  Future<_WindowState?> _loadState() async {
    final file = File(_stateFilePath());
    if (!await file.exists()) {
      return null;
    }
    try {
      final raw = await file.readAsString();
      final decoded = jsonDecode(raw);
      if (decoded is! Map<String, dynamic>) {
        return null;
      }
      final state = _WindowState.fromJson(decoded);
      if (!_isStateReasonable(state)) {
        return null;
      }
      return state;
    } catch (_) {
      return null;
    }
  }

  Future<void> _writeState(_WindowState state) async {
    final path = _stateFilePath();
    final file = File(path);
    await file.parent.create(recursive: true);
    await file.writeAsString(jsonEncode(state.toJson()));
  }

  bool _isStateReasonable(_WindowState state) {
    if (state.width < 300 || state.height < 200) {
      return false;
    }
    if (!state.width.isFinite || !state.height.isFinite) {
      return false;
    }
    if (!state.x.isFinite || !state.y.isFinite) {
      return false;
    }
    return true;
  }

  String _stateFilePath() {
    final sep = Platform.pathSeparator;
    final baseDir = _appDataDir();
    return '$baseDir${sep}BaYin${sep}window_state.json';
  }

  String _appDataDir() {
    final sep = Platform.pathSeparator;
    if (Platform.isWindows) {
      return Platform.environment['APPDATA'] ?? Directory.current.path;
    }
    if (Platform.isMacOS) {
      final home = Platform.environment['HOME'] ?? Directory.current.path;
      return '$home${sep}Library${sep}Application Support';
    }
    if (Platform.isLinux) {
      final xdg = Platform.environment['XDG_DATA_HOME'];
      if (xdg != null && xdg.isNotEmpty) {
        return xdg;
      }
      final home = Platform.environment['HOME'] ?? Directory.current.path;
      return '$home$sep.local${sep}share';
    }
    return Directory.current.path;
  }
}

class _WindowState {
  const _WindowState({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
    required this.isMaximized,
  });

  factory _WindowState.fromJson(Map<String, dynamic> json) {
    return _WindowState(
      x: (json['x'] as num).toDouble(),
      y: (json['y'] as num).toDouble(),
      width: (json['width'] as num).toDouble(),
      height: (json['height'] as num).toDouble(),
      isMaximized: json['isMaximized'] as bool? ?? false,
    );
  }

  final double x;
  final double y;
  final double width;
  final double height;
  final bool isMaximized;

  Size get size => Size(width, height);

  Rect get bounds => Rect.fromLTWH(x, y, width, height);

  Map<String, dynamic> toJson() => <String, dynamic>{
        'x': x,
        'y': y,
        'width': width,
        'height': height,
        'isMaximized': isMaximized,
      };
}
