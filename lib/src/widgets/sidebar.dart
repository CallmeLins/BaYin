import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../i18n/strings.g.dart';
import '../providers/providers.dart';
import '../theme/design_tokens.dart';

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
    final brightness = Theme.of(context).brightness;

    final useMobileSidebarStyle = isOverlay && !layout.isTablet;
    final isDark = tokens.isDark;
    final sectionBg = FlatColors.muted(brightness);

    Widget sectionWrap({required Widget child}) {
      if (!useMobileSidebarStyle) return child;
      return Container(
        margin: const EdgeInsets.only(bottom: FlatSpacing.sm + 4),
        padding: const EdgeInsets.all(FlatSpacing.sm + 2),
        decoration: BoxDecoration(
          color: sectionBg,
          borderRadius: BorderRadius.circular(FlatRadius.md),
        ),
        child: child,
      );
    }

    return Container(
      color: isOverlay ? tokens.windowBg : tokens.sidebarBg,
      child: SafeArea(
        bottom: false,
        right: false,
        child: Column(
          children: [
            Padding(
              padding: EdgeInsets.fromLTRB(
                FlatSpacing.sm + 4,
                useMobileSidebarStyle ? FlatSpacing.sm + 4 : FlatSpacing.sm,
                FlatSpacing.sm + 4,
                useMobileSidebarStyle ? FlatSpacing.sm : FlatSpacing.sm - 2,
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
                  if (kIsWeb) return;
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
                padding: const EdgeInsets.fromLTRB(FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.sm + 4),
                child: Divider(
                  color: tokens.separatorSoftColor,
                  height: FlatBorder.structural,
                ),
              ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(FlatSpacing.sm, FlatSpacing.sm, FlatSpacing.sm, 0),
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
        const SizedBox(width: FlatSpacing.sm),
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

    final brightness = Theme.of(context).brightness;
    final cardBg = FlatColors.muted(brightness);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(FlatSpacing.sm),
      decoration: BoxDecoration(
        color: cardBg,
        borderRadius: BorderRadius.circular(FlatRadius.md),
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
    final brightness = Theme.of(context).brightness;
    final fg = widget.danger
        ? FlatColors.error(brightness)
        : FlatColors.textSecondary(brightness);
    final hoverBg = widget.danger
        ? FlatColors.error(brightness).withValues(alpha: 0.10)
        : FlatColors.overlayOnMuted(brightness);
    final radius = widget.mobileCardStyle ? FlatRadius.sm + 2 : FlatRadius.md;

    return Tooltip(
      message: widget.tooltip,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        cursor: SystemMouseCursors.click,
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
    final brightness = Theme.of(context).brightness;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(FlatSpacing.xs, 0, FlatSpacing.xs, FlatSpacing.sm),
          child: Row(
            children: [
              Text(
                title.toUpperCase(),
                style: FlatTypography.label(brightness),
              ),
              const SizedBox(width: FlatSpacing.sm),
              Expanded(
                child: Divider(
                  color: FlatColors.border(brightness),
                  height: 1,
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
    final brightness = Theme.of(context).brightness;
    final isDark = widget.isDark;
    final isActive = widget.isActive;
    final primary = FlatColors.primary(brightness);
    final hoverBg = FlatColors.overlayOnDark(brightness);
    final activeBg = isDark
        ? Colors.white.withValues(alpha: 0.10)
        : FlatColors.muted(brightness);
    final normalFg = FlatColors.textSecondary(brightness);
    final activeFg = FlatColors.foreground(brightness);
    final iconColor = isActive
        ? primary
        : FlatColors.textSecondary(brightness);
    final bgColor = isActive
        ? activeBg
        : (_hovering ? hoverBg : Colors.transparent);

    return Padding(
      padding: EdgeInsets.symmetric(
        vertical: widget.isMobileStyle ? FlatSpacing.xs / 2 : 1,
      ),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        cursor: SystemMouseCursors.click,
        child: GestureDetector(
          onTap: widget.onTap,
          child: Container(
            decoration: BoxDecoration(
              color: bgColor,
              borderRadius: BorderRadius.circular(FlatRadius.md),
            ),
            child: Stack(
              children: [
                Padding(
                  padding: EdgeInsets.symmetric(
                    horizontal: FlatSpacing.sm + 4,
                    vertical: widget.isMobileStyle
                        ? FlatSpacing.sm + 2
                        : FlatSpacing.sm,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        widget.icon,
                        size: widget.isMobileStyle ? 20 : 18,
                        color: iconColor,
                      ),
                      const SizedBox(width: FlatSpacing.sm + 4),
                      Expanded(
                        child: Text(
                          widget.label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: widget.isMobileStyle ? FlatSpacing.md : 14,
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
                    top: widget.isMobileStyle
                        ? FlatSpacing.sm + 2
                        : FlatSpacing.sm,
                    bottom: widget.isMobileStyle
                        ? FlatSpacing.sm + 2
                        : FlatSpacing.sm,
                    child: Container(
                      width: FlatBorder.structural + 1,
                      decoration: BoxDecoration(
                        color: primary,
                        borderRadius: const BorderRadius.horizontal(
                          right: Radius.circular(FlatRadius.sm),
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
