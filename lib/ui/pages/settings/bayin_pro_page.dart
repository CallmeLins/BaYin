import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class BayinProPage extends ConsumerWidget {
  const BayinProPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final app = ref.watch(appSettingsProvider);
    final controller = ref.read(appSettingsProvider.notifier);
    final brightness = Theme.of(context).brightness;

    return Column(
      children: [
        BayinPageHeader(
          title: const Text('BaYin Pro'),
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

              // ── Pricing Section ──────────────────────────────────────
              const BayinSectionHeader(title: 'PRICING'),
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Free',
                        style: TextStyle(
                          fontSize: 24,
                          fontWeight: FontWeight.w700,
                          color: brightness == Brightness.dark
                              ? Colors.white
                              : Colors.black,
                        ),
                      ),
                      const SizedBox(height: 8),
                      Text(
                        'BaYin Pro is free and open source.\nNo payment required.',
                        style: TextStyle(
                          fontSize: 14,
                          color: brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.65)
                              : Colors.black.withValues(alpha: 0.60),
                          height: 1.5,
                        ),
                      ),
                      const SizedBox(height: 16),
                      SizedBox(
                        width: double.infinity,
                        child: FilledButton.icon(
                          onPressed: () => context.go('/about/donate'),
                          style: FilledButton.styleFrom(
                            padding: const EdgeInsets.symmetric(vertical: 14),
                            shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(14),
                            ),
                            elevation: 0,
                          ),
                          icon: Icon(PhosphorIcons.coffee(), size: 18),
                          label: const Text(
                            'Buy Me a Coffee',
                            style: TextStyle(
                              fontSize: 15,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),

              const SizedBox(height: 24),

              // ── Activate Section ─────────────────────────────────────
              const BayinSectionHeader(title: 'ACTIVATE'),
              BayinGlassGroup(
                child: _SwitchRow(
                  title: 'Enable Pro',
                  subtitle: 'Master switch for all Pro-only options.',
                  value: app.proEnabled,
                  onChanged: controller.setProEnabled,
                ),
              ),

              const SizedBox(height: 24),

              // ── Settings Section ─────────────────────────────────────
              const BayinSectionHeader(title: 'SETTINGS'),
              BayinGlassGroup(
                child: Column(
                  children: [
                    _SwitchRow(
                      title: 'Desktop glass effect',
                      subtitle: 'Enable translucent desktop surface accents.',
                      value: app.proGlassEnabled,
                      onChanged: app.proEnabled
                          ? controller.setProGlassEnabled
                          : null,
                      locked: !app.proEnabled,
                    ),
                    _InsetDivider(),
                    _SwitchRow(
                      title: 'Colorful spectrum',
                      subtitle: 'Allow richer color rendering for spectrum views.',
                      value: app.proColorSpectrumEnabled,
                      onChanged: app.proEnabled
                          ? controller.setProColorSpectrumEnabled
                          : null,
                      locked: !app.proEnabled,
                    ),
                    _InsetDivider(),
                    _SwitchRow(
                      title: 'Pure mode feature',
                      subtitle: 'Unlock Pure mode toggle in playback experience.',
                      value: app.proPureModeEnabled,
                      onChanged: app.proEnabled
                          ? controller.setProPureModeFeatureEnabled
                          : null,
                      locked: !app.proEnabled,
                    ),
                    _InsetDivider(),
                    _SwitchRow(
                      title: 'Pure mode active',
                      subtitle: 'Hide non-essential UI during playback.',
                      value: app.pureModeEnabled,
                      onChanged: app.proEnabled && app.proPureModeEnabled
                          ? controller.setPureModeEnabled
                          : null,
                      locked: !app.proEnabled || !app.proPureModeEnabled,
                    ),
                  ],
                ),
              ),

              const SizedBox(height: 24),

              // ── More Coming ──────────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Center(
                    child: Text(
                      'More features coming soon.',
                      style: TextStyle(
                        fontSize: 14,
                        color: brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.55)
                            : Colors.black.withValues(alpha: 0.50),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _SwitchRow extends StatelessWidget {
  const _SwitchRow({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
    this.locked = false,
  });

  final String title;
  final String subtitle;
  final bool value;
  final Future<void> Function(bool)? onChanged;
  final bool locked;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return InkWell(
      onTap: onChanged == null ? null : () => onChanged!(!value),
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
        child: Row(
          children: [
            if (locked) ...[
              Icon(
                PhosphorIcons.lock(),
                size: 14,
                color: brightness == Brightness.dark
                    ? Colors.white.withValues(alpha: 0.35)
                    : Colors.black.withValues(alpha: 0.30),
              ),
              const SizedBox(width: 8),
            ],
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
              onChanged: onChanged == null ? null : (v) => onChanged!(v),
            ),
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
