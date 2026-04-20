import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';

const double _winTitlebarHeight = 32;
const double _macTitlebarHeight = 28;

/// Platform-conditional titlebar.
///
/// * Windows / Linux desktop → custom titlebar with minimize/maximize/close
/// * macOS desktop          → 28 px drag region (traffic lights render natively
///                            via `TitleBarStyle.hidden` + overlay)
/// * Mobile / web           → zero-height SizedBox
class AppTitlebar extends ConsumerStatefulWidget {
  const AppTitlebar({super.key});

  static bool get supportsCustomChrome =>
      !kIsWeb && (Platform.isWindows || Platform.isLinux || Platform.isMacOS);

  static double heightFor({required bool isMacOS}) =>
      isMacOS ? _macTitlebarHeight : _winTitlebarHeight;

  @override
  ConsumerState<AppTitlebar> createState() => _AppTitlebarState();
}

class _AppTitlebarState extends ConsumerState<AppTitlebar> with WindowListener {
  bool _isMaximized = false;

  @override
  void initState() {
    super.initState();
    if (AppTitlebar.supportsCustomChrome) {
      windowManager.addListener(this);
      _refreshMaximizedState();
    }
  }

  @override
  void dispose() {
    if (AppTitlebar.supportsCustomChrome) {
      windowManager.removeListener(this);
    }
    super.dispose();
  }

  Future<void> _refreshMaximizedState() async {
    final value = await windowManager.isMaximized();
    if (!mounted) return;
    if (value != _isMaximized) setState(() => _isMaximized = value);
  }

  @override
  void onWindowMaximize() => _refreshMaximizedState();

  @override
  void onWindowUnmaximize() => _refreshMaximizedState();

  @override
  void onWindowResize() => _refreshMaximizedState();

  @override
  Widget build(BuildContext context) {
    if (!AppTitlebar.supportsCustomChrome) {
      return const SizedBox.shrink();
    }
    final platform = ref.watch(platformProvider);
    final tokens = Theme.of(context).extension<BayinTokens>()!;

    if (platform.isMacOS) {
      return SizedBox(
        height: _macTitlebarHeight,
        child: DragToMoveArea(
          child: Container(color: tokens.titlebarBg),
        ),
      );
    }

    return SizedBox(
      height: _winTitlebarHeight,
      child: Container(
        color: tokens.titlebarBg,
        child: Row(
          children: [
            Expanded(
              child: DragToMoveArea(
                child: Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: Padding(
                    padding: const EdgeInsets.only(left: 12),
                    child: Text(
                      'BaYin',
                      style: TextStyle(
                        fontSize: 12,
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ),
              ),
            ),
            _WindowButton(
              icon: PhosphorIcons.minus(),
              tooltip: 'Minimize',
              onTap: windowManager.minimize,
            ),
            _WindowButton(
              icon: _isMaximized
                  ? PhosphorIcons.copy()
                  : PhosphorIcons.square(),
              tooltip: _isMaximized ? 'Restore' : 'Maximize',
              onTap: () async {
                if (await windowManager.isMaximized()) {
                  await windowManager.unmaximize();
                } else {
                  await windowManager.maximize();
                }
              },
            ),
            _WindowButton(
              icon: PhosphorIcons.x(),
              tooltip: 'Close',
              isDanger: true,
              onTap: windowManager.close,
            ),
          ],
        ),
      ),
    );
  }
}

class _WindowButton extends StatefulWidget {
  const _WindowButton({
    required this.icon,
    required this.tooltip,
    required this.onTap,
    this.isDanger = false,
  });

  final IconData icon;
  final String tooltip;
  final Future<void> Function() onTap;
  final bool isDanger;

  @override
  State<_WindowButton> createState() => _WindowButtonState();
}

class _WindowButtonState extends State<_WindowButton> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final hoverBg = widget.isDanger
        ? const Color(0xFFE81123)
        : scheme.onSurface.withValues(alpha: 0.08);
    final hoverFg =
        widget.isDanger ? Colors.white : scheme.onSurface;

    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Tooltip(
        message: widget.tooltip,
        waitDuration: const Duration(milliseconds: 500),
        child: InkWell(
          onTap: () => widget.onTap(),
          child: Container(
            width: 44,
            height: _winTitlebarHeight,
            color: _hover ? hoverBg : Colors.transparent,
            alignment: Alignment.center,
            child: Icon(
              widget.icon,
              size: 14,
              color: _hover ? hoverFg : scheme.onSurfaceVariant,
            ),
          ),
        ),
      ),
    );
  }
}
