import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
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
          left: Tooltip(
            message: 'Back',
            child: IconButton(
              onPressed: () {
                if (context.canPop()) {
                  context.pop();
                } else {
                  context.go('/settings');
                }
              },
              icon: Icon(
                PhosphorIcons.caretLeft(),
                size: 20,
                color: FlatColors.textSecondary(brightness),
              ),
            ),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 20),
            children: [
              const SizedBox(height: 12),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Text(
                  'Feature flags for advanced visuals and focused playback.',
                  style: TextStyle(color: FlatColors.textSecondary(brightness)),
                ),
              ),
              const SizedBox(height: 24),
              _SectionGroup(
                title: 'PRO FEATURES',
                child: Column(
                  children: [
                    _SwitchTileCard(
                      title: 'Enable Pro',
                      subtitle: 'Master switch for all Pro-only options.',
                      value: app.proEnabled,
                      onChanged: controller.setProEnabled,
                    ),
                    _FlatDivider(),
                    _SwitchTileCard(
                      title: 'Desktop glass effect',
                      subtitle: 'Enable translucent desktop surface accents.',
                      value: app.proGlassEnabled,
                      onChanged: app.proEnabled ? controller.setProGlassEnabled : null,
                    ),
                    _FlatDivider(),
                    _SwitchTileCard(
                      title: 'Colorful spectrum',
                      subtitle: 'Allow richer color rendering for spectrum views.',
                      value: app.proColorSpectrumEnabled,
                      onChanged:
                          app.proEnabled ? controller.setProColorSpectrumEnabled : null,
                    ),
                    _FlatDivider(),
                    _SwitchTileCard(
                      title: 'Pure mode feature',
                      subtitle: 'Unlock Pure mode toggle in playback experience.',
                      value: app.proPureModeEnabled,
                      onChanged:
                          app.proEnabled ? controller.setProPureModeFeatureEnabled : null,
                    ),
                    _FlatDivider(),
                    _SwitchTileCard(
                      title: 'Pure mode active',
                      subtitle: 'Hide non-essential UI during playback.',
                      value: app.pureModeEnabled,
                      onChanged: app.proEnabled && app.proPureModeEnabled
                          ? controller.setPureModeEnabled
                          : null,
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

class _SectionGroup extends ConsumerWidget {
  const _SectionGroup({required this.title, required this.child});

  final String title;
  final Widget child;

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
          child: child,
        ),
      ],
    );
  }
}

class _SwitchTileCard extends ConsumerWidget {
  const _SwitchTileCard({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });

  final String title;
  final String subtitle;
  final bool value;
  final Future<void> Function(bool)? onChanged;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return InkWell(
      onTap: onChanged == null ? null : () => onChanged!(!value),
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
                    style: FlatTypography.bodySmall(brightness).copyWith(
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
            Switch(
              value: value,
              onChanged: onChanged == null ? null : (v) => onChanged!(v),
            ),
          ],
        ),
      ),
    );
  }
}
