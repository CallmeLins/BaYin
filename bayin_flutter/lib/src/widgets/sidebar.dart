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

const double kSidebarDockedWidth = 256; // matches Tailwind w-64
const double kSidebarOverlayWidthFraction = 0.6; // 60% viewport width

/// Sidebar nav — mirrors `src-ui/src/components/Sidebar.tsx`.
///
/// Two render modes controlled by the parent (usually RootScaffold):
///   * docked — fixed 256 px column inside a Row; no slide animation
///   * overlay — absolute-positioned drawer that slides in from the left
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
    final scheme = Theme.of(context).colorScheme;
    final t = context.t;
    final currentPath = GoRouterState.of(context).matchedLocation;

    final librarySection = _NavSection(
      title: t.nav.library,
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
    );

    final systemSection = _NavSection(
      title: t.nav.system,
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
    );

    return Material(
      color: isOverlay ? tokens.windowBg : tokens.sidebarBg,
      child: SafeArea(
        right: false,
        child: Column(
          children: [
            _TopActions(onNavigate: onNavigate),
            Divider(color: tokens.separatorColor, height: 1, thickness: 0.5),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.symmetric(vertical: 8),
                children: [
                  librarySection.build(context, currentPath, onNavigate),
                  const SizedBox(height: 16),
                  systemSection.build(context, currentPath, onNavigate),
                  const SizedBox(height: 16),
                  _DevOnlyDebugLink(
                    currentPath: currentPath,
                    onNavigate: onNavigate,
                    fg: scheme.onSurfaceVariant,
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

class _TopActions extends ConsumerWidget {
  const _TopActions({required this.onNavigate});

  final VoidCallback? onNavigate;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final themeMode = ref.watch(themeModeProvider);
    final t = context.t;
    final isDark = Theme.of(context).brightness == Brightness.dark;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Row(
        children: [
          IconButton(
            tooltip: t.common.toggleDarkMode,
            icon: Icon(isDark ? PhosphorIcons.sun() : PhosphorIcons.moon()),
            onPressed: () {
              final next = themeMode == ThemeMode.dark
                  ? ThemeMode.light
                  : ThemeMode.dark;
              ref.read(themeModeProvider.notifier).setThemeMode(next);
            },
          ),
          IconButton(
            tooltip: t.common.exit,
            icon: Icon(PhosphorIcons.signOut()),
            onPressed: () async {
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
          ),
        ],
      ),
    );
  }
}

class _DevOnlyDebugLink extends StatelessWidget {
  const _DevOnlyDebugLink({
    required this.currentPath,
    required this.onNavigate,
    required this.fg,
  });

  final String currentPath;
  final VoidCallback? onNavigate;
  final Color fg;

  @override
  Widget build(BuildContext context) {
    final isActive = currentPath == '/debug';
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: _NavListTile(
        icon: PhosphorIcons.bug(),
        label: 'Debug (FFI)',
        isActive: isActive,
        onTap: () {
          context.go('/debug');
          onNavigate?.call();
        },
      ),
    );
  }
}

class _NavSection {
  _NavSection({required this.title, required this.items});

  final String title;
  final List<_NavItem> items;

  Widget build(BuildContext context, String currentPath, VoidCallback? onNavigate) {
    final fg = Theme.of(context).colorScheme.onSurfaceVariant;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 4, 12, 6),
            child: Text(
              title.toUpperCase(),
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: fg,
                letterSpacing: 0.4,
              ),
            ),
          ),
          for (final item in items)
            _NavListTile(
              icon: item.icon,
              label: item.label,
              isActive: currentPath == item.path,
              onTap: () {
                context.go(item.path);
                onNavigate?.call();
              },
            ),
        ],
      ),
    );
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
    required this.isActive,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool isActive;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final bg = isActive ? scheme.primary.withValues(alpha: 0.08) : null;
    final fg = isActive ? scheme.primary : scheme.onSurface;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 1),
      child: Material(
        color: bg ?? Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            child: Row(
              children: [
                Icon(icon, size: 18, color: fg),
                const SizedBox(width: 12),
                Text(
                  label,
                  style: TextStyle(
                    fontSize: 13.5,
                    color: fg,
                    fontWeight:
                        isActive ? FontWeight.w600 : FontWeight.w400,
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
