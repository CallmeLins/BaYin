import 'package:flutter/foundation.dart';
import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../i18n/strings.g.dart';
import '../providers/providers.dart';

const double kSidebarDockedWidth = 256;
const double kSidebarOverlayWidthFraction = 0.6;

class Sidebar extends ConsumerWidget {
  const Sidebar({
    super.key,
    required this.isOverlay,
    this.onNavigate,
  });

  final bool isOverlay;
  final VoidCallback? onNavigate;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final t = context.t;
    final layout = ref.watch(responsiveLayoutProvider);
    final themeMode = ref.watch(themeModeProvider);
    final currentPath = GoRouterState.of(context).uri.path;

    final useMobileSidebarStyle = isOverlay && !layout.isTablet;
    final isDark = tokens.isDark;
    final sectionBg = isDark
        ? Colors.white.withValues(alpha: 0.05)
        : Colors.black.withValues(alpha: 0.04);

    Widget sectionWrap({required Widget child}) {
      if (!useMobileSidebarStyle) return child;
      return Container(
        margin: const EdgeInsets.only(bottom: 12),
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: sectionBg,
          borderRadius: BorderRadius.circular(12),
        ),
        child: child,
      );
    }

    return Container(
      color: isOverlay
          ? tokens.windowBg
          : tokens.sidebarBg.withValues(alpha: 0.70),
      child: SafeArea(
        bottom: false,
        right: false,
        child: Column(
          children: [
            Padding(
              padding: EdgeInsets.fromLTRB(
                12,
                useMobileSidebarStyle ? 12 : 8,
                12,
                useMobileSidebarStyle ? 8 : 6,
              ),
              child: _TopActions(
                mobileCardStyle: useMobileSidebarStyle,
                isDark: isDark,
                onToggleTheme: () {
                  final next = themeMode == ThemeMode.dark
                      ? ThemeMode.light
                      : ThemeMode.dark;
                  ref.read(themeModeProvider.notifier).setThemeMode(next);
                },
                onExit: () async {
                  onNavigate?.call();
                  if (kIsWeb) {
                    return;
                  }
                  final isDesktop = switch (defaultTargetPlatform) {
                    TargetPlatform.windows => true,
                    TargetPlatform.linux => true,
                    TargetPlatform.macOS => true,
                    _ => false,
                  };
                  if (isDesktop) {
                    await windowManager.close();
                    return;
                  }
                  await SystemNavigator.pop();
                },
                darkModeLabel: t.common.toggleDarkMode,
                exitLabel: t.common.exit,
              ),
            ),
            if (!useMobileSidebarStyle)
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
                child: Container(
                  height: 1,
                  decoration: BoxDecoration(
                    gradient: LinearGradient(
                      colors: [
                        Colors.transparent,
                        tokens.separatorColor,
                        Colors.transparent,
                      ],
                    ),
                  ),
                ),
              ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(8, 8, 8, 0),
                children: [
                  sectionWrap(
                    child: _NavSection(
                      title: t.nav.library,
                      useMobileStyle: useMobileSidebarStyle,
                      isDark: isDark,
                      items: [
                        _NavItem(
                          icon: PhosphorIcons.musicNote(),
                          label: t.nav.songs,
                          path: '/',
                        ),
                        _NavItem(
                          icon: PhosphorIcons.vinylRecord(),
                          label: t.nav.albums,
                          path: '/albums',
                        ),
                        _NavItem(
                          icon: PhosphorIcons.microphone(),
                          label: t.nav.artists,
                          path: '/artists',
                        ),
                        _NavItem(
                          icon: PhosphorIcons.playlist(),
                          label: t.nav.playlists,
                          path: '/playlists',
                        ),
                      ],
                      currentPath: currentPath,
                      onNavigate: onNavigate,
                    ),
                  ),
                  sectionWrap(
                    child: _NavSection(
                      title: t.nav.system,
                      useMobileStyle: useMobileSidebarStyle,
                      isDark: isDark,
                      items: [
                        _NavItem(
                          icon: PhosphorIcons.folderSimple(),
                          label: t.nav.scanMusic,
                          path: '/scan',
                        ),
                        _NavItem(
                          icon: PhosphorIcons.database(),
                          label: t.nav.libraryStats,
                          path: '/library',
                        ),
                        _NavItem(
                          icon: PhosphorIcons.gearSix(),
                          label: t.nav.settings,
                          path: '/settings',
                        ),
                        _NavItem(
                          icon: PhosphorIcons.info(),
                          label: t.nav.about,
                          path: '/about',
                        ),
                      ],
                      currentPath: currentPath,
                      onNavigate: onNavigate,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _TopActions extends StatelessWidget {
  const _TopActions({
    required this.mobileCardStyle,
    required this.isDark,
    required this.onToggleTheme,
    required this.onExit,
    required this.darkModeLabel,
    required this.exitLabel,
  });

  final bool mobileCardStyle;
  final bool isDark;
  final VoidCallback onToggleTheme;
  final Future<void> Function() onExit;
  final String darkModeLabel;
  final String exitLabel;

  @override
  Widget build(BuildContext context) {
    final content = Row(
      children: [
        _ActionButton(
          icon: isDark ? PhosphorIcons.sun() : PhosphorIcons.moon(),
          onTap: onToggleTheme,
          tooltip: darkModeLabel,
          isDark: isDark,
          mobileCardStyle: mobileCardStyle,
        ),
        const SizedBox(width: 8),
        _ActionButton(
          icon: PhosphorIcons.signOut(),
          onTap: () => onExit(),
          tooltip: exitLabel,
          isDark: isDark,
          mobileCardStyle: mobileCardStyle,
          danger: true,
        ),
      ],
    );

    if (!mobileCardStyle) {
      return content;
    }

    final cardBg = isDark
        ? Colors.white.withValues(alpha: 0.05)
        : Colors.black.withValues(alpha: 0.05);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: cardBg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: content,
    );
  }
}

class _ActionButton extends StatefulWidget {
  const _ActionButton({
    required this.icon,
    required this.onTap,
    required this.tooltip,
    required this.isDark,
    required this.mobileCardStyle,
    this.danger = false,
  });

  final IconData icon;
  final VoidCallback onTap;
  final String tooltip;
  final bool isDark;
  final bool mobileCardStyle;
  final bool danger;

  @override
  State<_ActionButton> createState() => _ActionButtonState();
}

class _ActionButtonState extends State<_ActionButton> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final fg = widget.danger
        ? (widget.isDark ? const Color(0xFFF87171) : const Color(0xFFDC2626))
        : (widget.isDark
            ? Colors.white.withValues(alpha: 0.70)
            : Colors.black.withValues(alpha: 0.54));
    final hoverBg = widget.danger
        ? const Color(0xFFEF4444).withValues(alpha: 0.10)
        : (widget.isDark
            ? Colors.white.withValues(alpha: 0.08)
            : Colors.black.withValues(alpha: 0.06));
    final radius = widget.mobileCardStyle ? 10.0 : 8.0;

    return Tooltip(
      message: widget.tooltip,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        child: GestureDetector(
          onTap: widget.onTap,
          child: Container(
            width: widget.mobileCardStyle ? 40 : 38,
            height: widget.mobileCardStyle ? 40 : 38,
            decoration: BoxDecoration(
              color: _hovering ? hoverBg : Colors.transparent,
              borderRadius: BorderRadius.circular(radius),
            ),
            child: Icon(widget.icon,
                size: widget.mobileCardStyle ? 20 : 18, color: fg),
          ),
        ),
      ),
    );
  }
}

class _NavSection extends StatelessWidget {
  const _NavSection({
    required this.title,
    required this.useMobileStyle,
    required this.isDark,
    required this.items,
    required this.currentPath,
    required this.onNavigate,
  });

  final String title;
  final bool useMobileStyle;
  final bool isDark;
  final List<_NavItem> items;
  final String currentPath;
  final VoidCallback? onNavigate;

  @override
  Widget build(BuildContext context) {
    final titleColor = isDark
        ? Colors.white.withValues(alpha: 0.45)
        : Colors.black.withValues(alpha: 0.45);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(4, 0, 4, 8),
          child: Row(
            children: [
              Text(
                title.toUpperCase(),
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: titleColor,
                  letterSpacing: 0.35,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Container(
                  height: 1,
                  color: isDark
                      ? Colors.white.withValues(alpha: 0.08)
                      : Colors.black.withValues(alpha: 0.08),
                ),
              ),
            ],
          ),
        ),
        for (final item in items)
          _NavListTile(
            icon: item.icon,
            label: item.label,
            isMobileStyle: useMobileStyle,
            isActive: _pathActive(currentPath, item.path),
            isDark: isDark,
            onTap: () {
              context.go(item.path);
              onNavigate?.call();
            },
          ),
      ],
    );
  }

  bool _pathActive(String currentPath, String targetPath) {
    if (targetPath == '/') {
      return currentPath == '/';
    }
    return currentPath == targetPath || currentPath.startsWith('$targetPath/');
  }
}

class _NavItem {
  const _NavItem({required this.icon, required this.label, required this.path});

  final IconData icon;
  final String label;
  final String path;
}

class _NavListTile extends StatefulWidget {
  const _NavListTile({
    required this.icon,
    required this.label,
    required this.isMobileStyle,
    required this.isActive,
    required this.isDark,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool isMobileStyle;
  final bool isActive;
  final bool isDark;
  final VoidCallback onTap;

  @override
  State<_NavListTile> createState() => _NavListTileState();
}

class _NavListTileState extends State<_NavListTile> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final isDark = widget.isDark;
    final isActive = widget.isActive;
    final hoverBg = isDark
        ? Colors.white.withValues(alpha: 0.04)
        : Colors.black.withValues(alpha: 0.03);
    final activeBg = isDark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.05);
    final normalFg = isDark
        ? Colors.white.withValues(alpha: 0.82)
        : Colors.black.withValues(alpha: 0.72);
    final activeFg = isDark
        ? Colors.white
        : Colors.black.withValues(alpha: 0.87);
    final iconColor = isActive
        ? const Color(0xFF3B82F6)
        : (isDark
            ? Colors.white.withValues(alpha: 0.54)
            : Colors.black.withValues(alpha: 0.45));
    final bgColor = isActive
        ? activeBg
        : (_hovering ? hoverBg : Colors.transparent);

    return Padding(
      padding: EdgeInsets.symmetric(vertical: widget.isMobileStyle ? 2 : 1),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        child: GestureDetector(
          onTap: widget.onTap,
          child: Container(
            decoration: BoxDecoration(
              color: bgColor,
              borderRadius: BorderRadius.circular(8),
            ),
            child: Stack(
              children: [
                Padding(
                  padding: EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: widget.isMobileStyle ? 10 : 8,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        widget.icon,
                        size: widget.isMobileStyle ? 20 : 18,
                        color: iconColor,
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          widget.label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: widget.isMobileStyle ? 16 : 14,
                            fontWeight:
                                isActive ? FontWeight.w600 : FontWeight.w500,
                            color: isActive ? activeFg : normalFg,
                            letterSpacing: 0.1,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                if (isActive)
                  Positioned(
                    left: 0,
                    top: widget.isMobileStyle ? 10 : 8,
                    bottom: widget.isMobileStyle ? 10 : 8,
                    child: Container(
                      width: 3,
                      decoration: const BoxDecoration(
                        color: Color(0xFF3B82F6),
                        borderRadius: BorderRadius.horizontal(
                          right: Radius.circular(3),
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
