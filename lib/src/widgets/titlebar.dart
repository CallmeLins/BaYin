import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../i18n/strings.g.dart';
import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';
import '../theme/design_tokens.dart';

const double _winTitlebarHeight = 32;
const double _macTitlebarHeight = 28;

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
    final t = context.t;
    final platform = ref.watch(platformProvider);
    final tokens = ref.watch(bayinTokensProvider);

    if (platform.isMacOS) {
      return SizedBox(
        height: _macTitlebarHeight,
        child: GestureDetector(
          onDoubleTap: _toggleMaximize,
          child: DragToMoveArea(
            child: Container(
              color: tokens.titlebarBg,
              padding: const EdgeInsets.only(left: 72, right: FlatSpacing.sm + 4),
              alignment: Alignment.center,
              child: Text(
                'BaYin',
                style: TextStyle(
                  fontSize: 12,
                  color: tokens.textSecondary,
                ),
              ),
            ),
          ),
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
              child: GestureDetector(
                onDoubleTap: _toggleMaximize,
                child: DragToMoveArea(
                  child: Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: Padding(
                      padding: const EdgeInsets.only(left: FlatSpacing.sm + 4),
                      child: Text(
                        'BaYin',
                        style: TextStyle(
                          fontSize: 12,
                          color: tokens.textSecondary,
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
            _WindowButton(
              icon: PhosphorIcons.minus(),
              tooltip: t.common.minimize,
              tokens: tokens,
              onTap: windowManager.minimize,
            ),
            _WindowButton(
              icon: _isMaximized
                  ? PhosphorIcons.copy()
                  : PhosphorIcons.square(),
              tooltip: _isMaximized ? t.common.restore : t.common.maximize,
              tokens: tokens,
              onTap: _toggleMaximize,
            ),
            _WindowButton(
              icon: PhosphorIcons.x(),
              tooltip: t.common.closeWindow,
              tokens: tokens,
              isDanger: true,
              onTap: windowManager.close,
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _toggleMaximize() async {
    if (await windowManager.isMaximized()) {
      await windowManager.unmaximize();
    } else {
      await windowManager.maximize();
    }
  }
}

class _WindowButton extends StatefulWidget {
  const _WindowButton({
    required this.icon,
    required this.tooltip,
    required this.tokens,
    required this.onTap,
    this.isDanger = false,
  });

  final IconData icon;
  final String tooltip;
  final BayinTokens tokens;
  final Future<void> Function() onTap;
  final bool isDanger;

  @override
  State<_WindowButton> createState() => _WindowButtonState();
}

class _WindowButtonState extends State<_WindowButton> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final hoverBg = widget.isDanger
        ? const Color(0xFFE81123)
        : Colors.white.withValues(alpha: 0.08);
    final hoverFg =
        widget.isDanger ? Colors.white : widget.tokens.textPrimary;

    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      cursor: SystemMouseCursors.click,
      child: Tooltip(
        message: widget.tooltip,
        child: GestureDetector(
          onTap: () => widget.onTap(),
          child: Container(
            width: 44,
            height: _winTitlebarHeight,
            color: _hover ? hoverBg : Colors.transparent,
            alignment: Alignment.center,
            child: Icon(
              widget.icon,
              size: 14,
              color: _hover ? hoverFg : widget.tokens.textSecondary,
            ),
          ),
        ),
      ),
    );
  }
}
