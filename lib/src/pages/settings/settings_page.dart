import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../i18n/strings.g.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final t = context.t;
    final themeMode = ref.watch(themeModeProvider);
    final locale = ref.watch(localeProvider);
    final app = ref.watch(appSettingsProvider);
    return Column(
      children: [
        BayinPageHeader(
          title: Text(t.nav.settings),
          right: Tooltip(
            message: 'About',
            child: IconButton(
              onPressed: () => context.go('/about'),
              icon: Icon(PhosphorIcons.info()),
            ),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 2),
              _SummaryCard(
                themeMode: _themeLabel(themeMode),
                localeLabel: _localeLabel(locale),
                eqEnabled: app.eqEnabled,
              ),
              const SizedBox(height: 12),
              _GroupCard(
                title: 'Preferences',
                tiles: [
                  _SettingsTileData(
                    icon: PhosphorIcons.palette(),
                    title: 'User interface',
                    subtitle: 'Theme, language and list presentation',
                    route: '/settings/interface',
                  ),
                  _SettingsTileData(
                    icon: PhosphorIcons.textAa(),
                    title: 'Lyrics',
                    subtitle: 'Lyrics typography and animation',
                    route: '/settings/lyrics',
                  ),
                  _SettingsTileData(
                    icon: PhosphorIcons.sliders(),
                    title: 'Equalizer',
                    subtitle: '10-band EQ profiles and gains',
                    route: '/settings/equalizer',
                  ),
                ],
              ),
              const SizedBox(height: 12),
              _GroupCard(
                title: 'Product',
                tiles: [
                  _SettingsTileData(
                    icon: PhosphorIcons.sparkle(),
                    title: 'BaYin Pro',
                    subtitle: 'Feature switches and visual modes',
                    route: '/settings/pro',
                  ),
                  _SettingsTileData(
                    icon: PhosphorIcons.chatTeardropText(),
                    title: 'Help & feedback',
                    subtitle: 'Troubleshooting and support info',
                    route: '/settings/help',
                  ),
                  _SettingsTileData(
                    icon: PhosphorIcons.downloadSimple(),
                    title: 'Update software',
                    subtitle: 'Version and update checks',
                    route: '/settings/update',
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }

  String _themeLabel(ThemeMode mode) {
    return switch (mode) {
      ThemeMode.system => 'System',
      ThemeMode.light => 'Light',
      ThemeMode.dark => 'Dark',
    };
  }

  String _localeLabel(Locale? locale) {
    if (locale == null) return 'System';
    if (locale.languageCode == 'zh') return 'Chinese (Simplified)';
    if (locale.languageCode == 'en') return 'English';
    return locale.toLanguageTag();
  }
}

class _SummaryCard extends ConsumerWidget {
  const _SummaryCard({
    required this.themeMode,
    required this.localeLabel,
    required this.eqEnabled,
  });

  final String themeMode;
  final String localeLabel;
  final bool eqEnabled;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final isDark = tokens.isDark;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: isDark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Wrap(
        spacing: 8,
        runSpacing: 8,
        children: [
          _pill('Theme: $themeMode', isDark),
          _pill('Language: $localeLabel', isDark),
          _pill('EQ: ${eqEnabled ? 'On' : 'Off'}', isDark),
        ],
      ),
    );
  }

  Widget _pill(String label, bool isDark) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: isDark
            ? Colors.white.withValues(alpha: 0.08)
            : Colors.black.withValues(alpha: 0.05),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 12,
          color: isDark
              ? const Color(0xFFF3F3F3)
              : const Color(0xFF0A0A0A),
        ),
      ),
    );
  }
}

class _GroupCard extends ConsumerWidget {
  const _GroupCard({
    required this.title,
    required this.tiles,
  });

  final String title;
  final List<_SettingsTileData> tiles;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final isDark = tokens.isDark;
    return Container(
      decoration: BoxDecoration(
        color: isDark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 6),
            child: Row(
              children: [
                Text(
                  title,
                  style: TextStyle(
                    fontWeight: FontWeight.w600,
                    color: tokens.textPrimary,
                  ),
                ),
              ],
            ),
          ),
          for (var i = 0; i < tiles.length; i++) ...[
            if (i > 0) Container(height: 1, color: tokens.separatorColor),
            _SettingsTile(data: tiles[i]),
          ],
        ],
      ),
    );
  }
}

class _SettingsTileData {
  const _SettingsTileData({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.route,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final String route;
}

class _SettingsTile extends ConsumerWidget {
  const _SettingsTile({required this.data});

  final _SettingsTileData data;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return GestureDetector(
      onTap: () => context.go(data.route),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
        child: Row(
          children: [
            Icon(data.icon, size: 18),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(data.title),
                  const SizedBox(height: 2),
                  Text(
                    data.subtitle,
                    style: TextStyle(
                      fontSize: 12,
                      color: tokens.textSecondary,
                    ),
                  ),
                ],
              ),
            ),
            Icon(PhosphorIcons.caretRight(), size: 16),
          ],
        ),
      ),
    );
  }
}
