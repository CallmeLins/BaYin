import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../i18n/strings.g.dart';
import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
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
            onPressed: () => context.go('/about'),
            icon: Icon(PhosphorIcons.info(), size: 20),
            tooltip: 'About',
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(
              FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xl,
            ),
            children: [
              const SizedBox(height: FlatSpacing.sm + 4),
              _SummaryCard(
                themeMode: _themeLabel(themeMode),
                localeLabel: _localeLabel(locale),
                eqEnabled: app.eqEnabled,
              ),
              const SizedBox(height: FlatSpacing.lg),
              _SettingsGroup(
                title: 'PREFERENCES',
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
              const SizedBox(height: FlatSpacing.lg),
              _SettingsGroup(
                title: 'PRODUCT',
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

/// Summary card showing current settings state as pills.
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
    final brightness = Theme.of(context).brightness;
    return Container(
      padding: const EdgeInsets.all(FlatSpacing.md),
      decoration: BoxDecoration(
        color: FlatColors.muted(brightness),
        borderRadius: BorderRadius.circular(FlatRadius.md),
      ),
      child: Wrap(
        spacing: FlatSpacing.sm,
        runSpacing: FlatSpacing.sm,
        children: [
          _Pill(label: 'Theme: $themeMode'),
          _Pill(label: 'Language: $localeLabel'),
          _Pill(label: 'EQ: ${eqEnabled ? 'On' : 'Off'}'),
        ],
      ),
    );
  }
}

class _Pill extends ConsumerWidget {
  const _Pill({required this.label});

  final String label;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: FlatSpacing.sm + 2,
        vertical: FlatSpacing.xs + 2,
      ),
      decoration: BoxDecoration(
        color: FlatColors.background(brightness),
        borderRadius: BorderRadius.circular(FlatRadius.md),
        border: Border.all(
          color: FlatColors.border(brightness),
          width: FlatBorder.structural,
        ),
      ),
      child: Text(
        label,
        style: FlatTypography.caption(brightness).copyWith(
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}

/// A settings tile data model.
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

/// A group of settings tiles with a section label.
class _SettingsGroup extends ConsumerWidget {
  const _SettingsGroup({
    required this.title,
    required this.tiles,
  });

  final String title;
  final List<_SettingsTileData> tiles;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xs + 2,
          ),
          child: Text(
            title,
            style: FlatTypography.label(brightness),
          ),
        ),
        Container(
          decoration: BoxDecoration(
            color: FlatColors.background(brightness),
            borderRadius: BorderRadius.circular(FlatRadius.md),
            border: Border.all(
              color: FlatColors.border(brightness),
              width: FlatBorder.structural,
            ),
          ),
          child: Column(
            children: [
              for (var i = 0; i < tiles.length; i++) ...[
                if (i > 0)
                  Divider(
                    height: FlatBorder.structural,
                    indent: FlatSpacing.xxl + FlatSpacing.sm,
                    color: FlatColors.border(brightness),
                  ),
                _SettingsTile(data: tiles[i]),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

/// Individual settings tile.
class _SettingsTile extends ConsumerWidget {
  const _SettingsTile({required this.data});

  final _SettingsTileData data;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return ListTile(
      onTap: () => context.go(data.route),
      leading: Icon(
        data.icon,
        size: 20,
        color: FlatColors.primary(brightness),
      ),
      title: Text(
        data.title,
        style: FlatTypography.bodySmall(brightness).copyWith(
          fontWeight: FontWeight.w500,
        ),
      ),
      subtitle: Text(
        data.subtitle,
        style: FlatTypography.caption(brightness),
      ),
      trailing: Icon(
        PhosphorIcons.caretRight(),
        size: 16,
        color: FlatColors.textSecondary(brightness),
      ),
    );
  }
}
