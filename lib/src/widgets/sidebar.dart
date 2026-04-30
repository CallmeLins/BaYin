import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../i18n/strings.g.dart';
import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';

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
    final tokens = Theme.of(context).extension<BayinTokens>()!;
    final t = context.t;
    final layout = ref.watch(responsiveLayoutProvider);
    final themeMode = ref.watch(themeModeProvider);
    final currentPath = GoRouterState.of(context).uri.path;

    final useMobileSidebarStyle = isOverlay && !layout.isTablet;
    final sectionBg = Theme.of(context).brightness == Brightness.dark
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

    return Material(
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
    required this.onToggleTheme,
    required this.onExit,
    required this.darkModeLabel,
    required this.exitLabel,
  });

  final bool mobileCardStyle;
  final VoidCallback onToggleTheme;
  final Future<void> Function() onExit;
  final String darkModeLabel;
  final String exitLabel;

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;

    Widget button({
      required IconData icon,
      required VoidCallback onTap,
      required String tooltip,
      bool danger = false,
    }) {
      final fg = danger
          ? (isDark ? const Color(0xFFF87171) : const Color(0xFFDC2626))
          : (isDark ? Colors.white70 : Colors.black54);
      final hoverBg = danger
          ? const Color(0xFFEF4444).withValues(alpha: 0.10)
          : (isDark
              ? Colors.white.withValues(alpha: 0.08)
              : Colors.black.withValues(alpha: 0.06));
      final radius = mobileCardStyle ? 10.0 : 8.0;

      return Tooltip(
        message: tooltip,
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(radius),
          child: InkWell(
            borderRadius: BorderRadius.circular(radius),
            onTap: onTap,
            hoverColor: hoverBg,
            child: Container(
              width: mobileCardStyle ? 40 : 38,
              height: mobileCardStyle ? 40 : 38,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(radius),
              ),
              child: Icon(icon, size: mobileCardStyle ? 20 : 18, color: fg),
            ),
          ),
        ),
      );
    }

    final content = Row(
      children: [
        button(
          icon: isDark ? PhosphorIcons.sun() : PhosphorIcons.moon(),
          onTap: onToggleTheme,
          tooltip: darkModeLabel,
        ),
        const SizedBox(width: 8),
        button(
          icon: PhosphorIcons.signOut(),
          onTap: () => onExit(),
          tooltip: exitLabel,
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

class _NavSection extends StatelessWidget {
  const _NavSection({
    required this.title,
    required this.useMobileStyle,
    required this.items,
    required this.currentPath,
    required this.onNavigate,
  });

  final String title;
  final bool useMobileStyle;
  final List<_NavItem> items;
  final String currentPath;
  final VoidCallback? onNavigate;

  @override
  Widget build(BuildContext context) {
    final titleColor = Theme.of(context).brightness == Brightness.dark
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
                  color: Theme.of(context).brightness == Brightness.dark
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

class _NavListTile extends StatelessWidget {
  const _NavListTile({
    required this.icon,
    required this.label,
    required this.isMobileStyle,
    required this.isActive,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool isMobileStyle;
  final bool isActive;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final activeBg = isDark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.05);
    final normalFg = isDark
        ? Colors.white.withValues(alpha: 0.82)
        : Colors.black.withValues(alpha: 0.72);
    final activeFg = isDark ? Colors.white : Colors.black87;
    final iconColor = isActive
        ? const Color(0xFF3B82F6)
        : (isDark ? Colors.white54 : Colors.black45);

    return Padding(
      padding: EdgeInsets.symmetric(vertical: isMobileStyle ? 2 : 1),
      child: Material(
        color: isActive ? activeBg : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Stack(
            children: [
              Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: isMobileStyle ? 10 : 8,
                ),
                child: Row(
                  children: [
                    Icon(
                      icon,
                      size: isMobileStyle ? 20 : 18,
                      color: iconColor,
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: isMobileStyle ? 16 : 14,
                          fontWeight: isActive ? FontWeight.w600 : FontWeight.w500,
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
                  top: isMobileStyle ? 10 : 8,
                  bottom: isMobileStyle ? 10 : 8,
                  child: Container(
                    width: 3,
                    decoration: BoxDecoration(
                      color: const Color(0xFF3B82F6),
                      borderRadius: const BorderRadius.horizontal(
                        right: Radius.circular(3),
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
