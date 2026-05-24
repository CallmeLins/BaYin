import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class UserInterfacePage extends ConsumerWidget {
  const UserInterfacePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final themeMode = ref.watch(themeModeProvider);
    final themeController = ref.read(themeModeProvider.notifier);
    final locale = ref.watch(localeProvider);
    final localeController = ref.read(localeProvider.notifier);
    final app = ref.watch(appSettingsProvider);
    final appController = ref.read(appSettingsProvider.notifier);

    return Column(
      children: [
        BayinPageHeader(
          title: const Text('User Interface'),
          left: BayinGhostIconButton(
            icon: PhosphorIcons.caretLeft(),
            tooltip: 'Back',
            onTap: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
              const BayinSectionHeader(title: 'APPEARANCE'),
              BayinGlassGroup(
                child: Column(
                  children: [
                    _SettingRow(
                      title: 'Theme',
                      trailing: SegmentedButton<ThemeMode>(
                        segments: const [
                          ButtonSegment<ThemeMode>(
                            value: ThemeMode.system,
                            label: Text('System'),
                          ),
                          ButtonSegment<ThemeMode>(
                            value: ThemeMode.light,
                            label: Text('Light'),
                          ),
                          ButtonSegment<ThemeMode>(
                            value: ThemeMode.dark,
                            label: Text('Dark'),
                          ),
                        ],
                        selected: {themeMode},
                        onSelectionChanged: (v) => themeController.setThemeMode(v.first),
                      ),
                    ),
                    _InsetDivider(),
                    _SettingRow(
                      title: 'Language',
                      trailing: DropdownButton<Locale?>(
                        value: locale,
                        items: const [
                          DropdownMenuItem<Locale?>(value: null, child: Text('System')),
                          DropdownMenuItem<Locale?>(value: Locale('en'), child: Text('English')),
                          DropdownMenuItem<Locale?>(
                            value: Locale('zh', 'CN'),
                            child: Text('Chinese (Simplified)'),
                          ),
                        ],
                        onChanged: localeController.setLocale,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 18),
              const BayinSectionHeader(title: 'LISTS'),
              BayinGlassGroup(
                child: Column(
                  children: [
                    _SwitchRow(
                      title: 'Show cover in lists',
                      subtitle: 'Display song art in row lists when available.',
                      value: app.showCoverInList,
                      onChanged: appController.setShowCoverInList,
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 18),
              const BayinSectionHeader(title: 'AUDIO VISUALIZER'),
              BayinGlassGroup(
                child: Column(
                  children: [
                    _SwitchRow(
                      title: 'Visualizer enabled',
                      subtitle: 'Enable audio visualizer rendering pipeline.',
                      value: app.visualizerEnabled,
                      onChanged: appController.setVisualizerEnabled,
                    ),
                    _InsetDivider(),
                    _SwitchRow(
                      title: 'Bass effect',
                      subtitle: 'Enable stronger low-frequency visual response.',
                      value: app.bassEffectEnabled,
                      onChanged: appController.setBassEffectEnabled,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}



class _SettingRow extends StatelessWidget {
  const _SettingRow({required this.title, required this.trailing});

  final String title;
  final Widget trailing;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              style: FlatTypography.bodySmall(brightness).copyWith(
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
          trailing,
        ],
      ),
    );
  }
}

class _SwitchRow extends StatelessWidget {
  const _SwitchRow({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });

  final String title;
  final String subtitle;
  final bool value;
  final Future<void> Function(bool) onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return InkWell(
      onTap: () => onChanged(!value),
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: FlatTypography.bodySmall(brightness).copyWith(
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(subtitle, style: FlatTypography.caption(brightness)),
                ],
              ),
            ),
            Switch(value: value, onChanged: onChanged),
          ],
        ),
      ),
    );
  }
}

class _InsetDivider extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.only(left: 50),
      child: Divider(
        height: 1,
        thickness: 1,
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.06)
            : Colors.black.withValues(alpha: 0.06),
      ),
    );
  }
}
