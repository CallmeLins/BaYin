import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/providers.dart';

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
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'User Interface',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 10),
        _SectionCard(
          title: 'Theme',
          child: SegmentedButton<ThemeMode>(
            segments: const [
              ButtonSegment(value: ThemeMode.system, label: Text('System')),
              ButtonSegment(value: ThemeMode.light, label: Text('Light')),
              ButtonSegment(value: ThemeMode.dark, label: Text('Dark')),
            ],
            selected: <ThemeMode>{themeMode},
            onSelectionChanged: (value) {
              if (value.isNotEmpty) {
                themeController.setThemeMode(value.first);
              }
            },
          ),
        ),
        const SizedBox(height: 10),
        _SectionCard(
          title: 'Language',
          child: DropdownButtonFormField<Locale?>(
            initialValue: locale,
            decoration: const InputDecoration(
              isDense: true,
              border: OutlineInputBorder(),
            ),
            items: const [
              DropdownMenuItem<Locale?>(
                value: null,
                child: Text('System'),
              ),
              DropdownMenuItem<Locale?>(
                value: Locale('en'),
                child: Text('English'),
              ),
              DropdownMenuItem<Locale?>(
                value: Locale('zh', 'CN'),
                child: Text('Chinese (Simplified)'),
              ),
            ],
            onChanged: localeController.setLocale,
          ),
        ),
        const SizedBox(height: 10),
        _SwitchTileCard(
          title: 'Show cover in lists',
          subtitle: 'Display song art in row lists when available.',
          value: app.showCoverInList,
          onChanged: appController.setShowCoverInList,
        ),
        const SizedBox(height: 10),
        _SwitchTileCard(
          title: 'Visualizer enabled',
          subtitle: 'Enable audio visualizer rendering pipeline.',
          value: app.visualizerEnabled,
          onChanged: appController.setVisualizerEnabled,
        ),
        const SizedBox(height: 10),
        _SwitchTileCard(
          title: 'Bass effect',
          subtitle: 'Enable stronger low-frequency visual response.',
          value: app.bassEffectEnabled,
          onChanged: appController.setBassEffectEnabled,
        ),
      ],
    );
  }
}

class _SectionCard extends StatelessWidget {
  const _SectionCard({
    required this.title,
    required this.child,
  });

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          child,
        ],
      ),
    );
  }
}

class _SwitchTileCard extends StatelessWidget {
  const _SwitchTileCard({
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
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: SwitchListTile.adaptive(
        value: value,
        onChanged: (v) => onChanged(v),
        title: Text(title),
        subtitle: Text(subtitle),
      ),
    );
  }
}
