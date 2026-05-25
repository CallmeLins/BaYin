import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';
import '../../widgets/toggle_switch.dart';

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
              const SizedBox(height: 16),

              // ── Appearance Section ───────────────────────────────────
              const BayinSectionHeader(title: 'APPEARANCE'),
              BayinGlassGroup(
                child: Column(
                  children: [
                    // ── Theme Selector ────────────────────────────────
                    Padding(
                      padding: const EdgeInsets.all(14),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text(
                            'Theme',
                            style: TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                          const SizedBox(height: 10),
                          _SegmentedSelector<ThemeMode>(
                            options: const [
                              _SegmentOption(value: ThemeMode.system, label: 'System'),
                              _SegmentOption(value: ThemeMode.light, label: 'Light'),
                              _SegmentOption(value: ThemeMode.dark, label: 'Dark'),
                            ],
                            selected: themeMode,
                            onChanged: themeController.setThemeMode,
                          ),
                        ],
                      ),
                    ),
                    _InsetDivider(),

                    // ── Language Selector ─────────────────────────────
                    Padding(
                      padding: const EdgeInsets.all(14),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text(
                            'Language',
                            style: TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                          const SizedBox(height: 10),
                          _SegmentedSelector<String?>(
                            options: const [
                              _SegmentOption(value: null, label: 'System'),
                              _SegmentOption(value: 'en', label: 'English'),
                              _SegmentOption(value: 'zh-CN', label: '中文'),
                            ],
                            selected: locale?.languageCode,
                            onChanged: (v) {
                              if (v == null) {
                                localeController.setLocale(null);
                              } else if (v == 'zh-CN') {
                                localeController.setLocale(const Locale('zh', 'CN'));
                              } else {
                                localeController.setLocale(const Locale('en'));
                              }
                            },
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),

              const SizedBox(height: 24),

              // ── Lists Section ────────────────────────────────────────
              const BayinSectionHeader(title: 'LISTS'),
              BayinGlassGroup(
                child: _SwitchRow(
                  title: 'Show cover in lists',
                  subtitle: 'Display song art in row lists when available.',
                  value: app.showCoverInList,
                  onChanged: appController.setShowCoverInList,
                ),
              ),

              const SizedBox(height: 24),

              // ── Audio Visualizer Section ─────────────────────────────
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

// ── Segmented Selector ───────────────────────────────────────────────────

class _SegmentOption<T> {
  const _SegmentOption({required this.value, required this.label});

  final T value;
  final String label;
}

class _SegmentedSelector<T> extends StatelessWidget {
  const _SegmentedSelector({
    required this.options,
    required this.selected,
    required this.onChanged,
  });

  final List<_SegmentOption<T>> options;
  final T selected;
  final ValueChanged<T> onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.06)
            : Colors.black.withValues(alpha: 0.04),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        children: [
          for (var i = 0; i < options.length; i++) ...[
            if (i > 0) const SizedBox(width: 4),
            Expanded(
              child: GestureDetector(
                onTap: () => onChanged(options[i].value),
                child: Container(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  decoration: BoxDecoration(
                    color: selected == options[i].value
                        ? (brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.14)
                            : Colors.white)
                        : Colors.transparent,
                    borderRadius: BorderRadius.circular(8),
                    boxShadow: selected == options[i].value
                        ? [
                            BoxShadow(
                              color: Colors.black.withValues(alpha: 0.06),
                              blurRadius: 4,
                              offset: const Offset(0, 1),
                            ),
                          ]
                        : null,
                  ),
                  alignment: Alignment.center,
                  child: Text(
                    options[i].label,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                      color: selected == options[i].value
                          ? (brightness == Brightness.dark
                              ? Colors.white
                              : Colors.black)
                          : (brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.55)
                              : Colors.black.withValues(alpha: 0.50)),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

// ── Switch Row ───────────────────────────────────────────────────────────

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
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
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
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: TextStyle(
                      fontSize: 12,
                      color: brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.45)
                          : Colors.black.withValues(alpha: 0.40),
                    ),
                  ),
                ],
              ),
            ),
            BayinToggleSwitch(
              value: value,
              onChanged: (v) => onChanged(v),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Inset Divider ────────────────────────────────────────────────────────

class _InsetDivider extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.only(left: 14),
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
