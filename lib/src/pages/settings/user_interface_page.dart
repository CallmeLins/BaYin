import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
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
            icon: Icon(PhosphorIcons.caretLeft()),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 10),
              _SectionCard(
                title: 'Theme',
                child: Row(
                  children: [
                    _themeChip('System', ThemeMode.system, themeMode, themeController.setThemeMode),
                    const SizedBox(width: 8),
                    _themeChip('Light', ThemeMode.light, themeMode, themeController.setThemeMode),
                    const SizedBox(width: 8),
                    _themeChip('Dark', ThemeMode.dark, themeMode, themeController.setThemeMode),
                  ],
                ),
              ),
              const SizedBox(height: 10),
              _SectionCard(
                title: 'Language',
                child: ComboBox<Locale?>(
                  value: locale,
                  items: const [
                    ComboBoxItem<Locale?>(value: null, child: Text('System')),
                    ComboBoxItem<Locale?>(value: Locale('en'), child: Text('English')),
                    ComboBoxItem<Locale?>(
                        value: Locale('zh', 'CN'), child: Text('Chinese (Simplified)')),
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
          ),
        ),
      ],
    );
  }
}

Widget _themeChip(
  String label,
  ThemeMode value,
  ThemeMode selected,
  Future<void> Function(ThemeMode) onSelected,
) {
  final active = selected == value;
  return GestureDetector(
    onTap: () => onSelected(value),
    child: Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      decoration: BoxDecoration(
        color: active
            ? const Color(0xFF3B82F6)
            : const Color(0x0A000000),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        label,
        style: TextStyle(
          color: active ? Colors.white : null,
          fontWeight: FontWeight.w500,
        ),
      ),
    ),
  );
}

class _SectionCard extends StatelessWidget {
  const _SectionCard({required this.title, required this.child});

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
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
    return BayinGlassCard(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
                  const SizedBox(height: 4),
                  Text(subtitle, style: const TextStyle(fontSize: 12)),
                ],
              ),
            ),
            ToggleSwitch(checked: value, onChanged: onChanged),
          ],
        ),
      ),
    );
  }
}
