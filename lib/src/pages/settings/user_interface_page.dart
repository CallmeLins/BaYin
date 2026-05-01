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
          left: IconButton(
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
            icon: Icon(PhosphorIcons.caretLeft(), size: 20),
            tooltip: 'Back',
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(
              FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xl,
            ),
            children: [
              const SizedBox(height: FlatSpacing.sm + 4),
              _SectionGroup(title: 'APPEARANCE', children: [
                _SettingRow(
                  title: 'Theme',
                  child: SegmentedButton<ThemeMode>(
                    segments: const [
                      ButtonSegment<ThemeMode>(
                        value: ThemeMode.system,
                        label: Text('System', style: TextStyle(fontSize: 13)),
                      ),
                      ButtonSegment<ThemeMode>(
                        value: ThemeMode.light,
                        label: Text('Light', style: TextStyle(fontSize: 13)),
                      ),
                      ButtonSegment<ThemeMode>(
                        value: ThemeMode.dark,
                        label: Text('Dark', style: TextStyle(fontSize: 13)),
                      ),
                    ],
                    selected: {themeMode},
                    onSelectionChanged: (v) => themeController.setThemeMode(v.first),
                  ),
                ),
              ]),
              const SizedBox(height: FlatSpacing.lg),
              _SectionGroup(title: 'LOCALIZATION', children: [
                _SettingRow(
                  title: 'Language',
                  child: DropdownButton<Locale?>(
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
              ]),
              const SizedBox(height: FlatSpacing.lg),
              _SectionGroup(title: 'LIST & VISUALIZER', children: [
                _SwitchTile(
                  title: 'Show cover in lists',
                  subtitle: 'Display song art in row lists when available.',
                  value: app.showCoverInList,
                  onChanged: appController.setShowCoverInList,
                ),
                _FlatDivider(),
                _SwitchTile(
                  title: 'Visualizer enabled',
                  subtitle: 'Enable audio visualizer rendering pipeline.',
                  value: app.visualizerEnabled,
                  onChanged: appController.setVisualizerEnabled,
                ),
                _FlatDivider(),
                _SwitchTile(
                  title: 'Bass effect',
                  subtitle: 'Enable stronger low-frequency visual response.',
                  value: app.bassEffectEnabled,
                  onChanged: appController.setBassEffectEnabled,
                ),
              ]),
            ],
          ),
        ),
      ],
    );
  }
}

/// A section group with a title label and grouped children.
class _SectionGroup extends ConsumerWidget {
  const _SectionGroup({required this.title, required this.children});

  final String title;
  final List<Widget> children;

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
          child: Text(title, style: FlatTypography.label(brightness)),
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
          child: Column(children: children),
        ),
      ],
    );
  }
}

/// A setting row with a title and trailing control.
class _SettingRow extends StatelessWidget {
  const _SettingRow({required this.title, required this.child});

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: FlatSpacing.md,
        vertical: FlatSpacing.sm + 4,
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            title,
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
          ),
          child,
        ],
      ),
    );
  }
}

/// A settings tile with a switch toggle.
class _SwitchTile extends StatelessWidget {
  const _SwitchTile({
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
      borderRadius: BorderRadius.circular(FlatRadius.md),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: FlatSpacing.md,
          vertical: FlatSpacing.sm + 4,
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: FlatSpacing.xs),
                  Text(
                    subtitle,
                    style: FlatTypography.caption(brightness),
                  ),
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

/// A structural 2px divider within a section group.
class _FlatDivider extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Divider(
      height: FlatBorder.structural,
      indent: FlatSpacing.md,
      color: FlatColors.border(brightness),
    );
  }
}
