import 'package:flutter/material.dart';
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
          right: IconButton(
            tooltip: 'About',
            onPressed: () => context.go('/about'),
            icon: Icon(PhosphorIcons.info()),
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

class _SummaryCard extends StatelessWidget {
  const _SummaryCard({
    required this.themeMode,
    required this.localeLabel,
    required this.eqEnabled,
  });

  final String themeMode;
  final String localeLabel;
  final bool eqEnabled;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Wrap(
        spacing: 8,
        runSpacing: 8,
        children: [
          _pill(context, 'Theme: $themeMode'),
          _pill(context, 'Language: $localeLabel'),
          _pill(context, 'EQ: ${eqEnabled ? 'On' : 'Off'}'),
        ],
      ),
    );
  }

  Widget _pill(BuildContext context, String label) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: scheme.secondaryContainer,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 12,
          color: scheme.onSecondaryContainer,
        ),
      ),
    );
  }
}

class _GroupCard extends StatelessWidget {
  const _GroupCard({
    required this.title,
    required this.tiles,
  });

  final String title;
  final List<_SettingsTileData> tiles;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
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
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
              ],
            ),
          ),
          for (var i = 0; i < tiles.length; i++) ...[
            if (i > 0) Divider(height: 1, color: scheme.outlineVariant),
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

class _SettingsTile extends StatelessWidget {
  const _SettingsTile({required this.data});

  final _SettingsTileData data;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: () => context.go(data.route),
        borderRadius: BorderRadius.circular(10),
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
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              Icon(PhosphorIcons.caretRight(), size: 16),
            ],
          ),
        ),
      ),
    );
  }
}
