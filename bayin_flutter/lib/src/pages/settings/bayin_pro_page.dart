import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/providers.dart';

class BayinProPage extends ConsumerWidget {
  const BayinProPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final app = ref.watch(appSettingsProvider);
    final controller = ref.read(appSettingsProvider.notifier);
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'BaYin Pro',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 6),
        Text(
          'Feature flags for advanced visuals and focused playback.',
          style: TextStyle(color: Theme.of(context).colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 12),
        _SwitchCard(
          title: 'Enable Pro',
          subtitle: 'Master switch for all Pro-only options.',
          value: app.proEnabled,
          onChanged: controller.setProEnabled,
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Desktop glass effect',
          subtitle: 'Enable translucent desktop surface accents.',
          value: app.proGlassEnabled,
          onChanged: app.proEnabled ? controller.setProGlassEnabled : null,
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Colorful spectrum',
          subtitle: 'Allow richer color rendering for spectrum views.',
          value: app.proColorSpectrumEnabled,
          onChanged: app.proEnabled
              ? controller.setProColorSpectrumEnabled
              : null,
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Pure mode feature',
          subtitle: 'Unlock Pure mode toggle in playback experience.',
          value: app.proPureModeEnabled,
          onChanged: app.proEnabled
              ? controller.setProPureModeFeatureEnabled
              : null,
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Pure mode active',
          subtitle: 'Hide non-essential UI during playback.',
          value: app.pureModeEnabled,
          onChanged: app.proEnabled && app.proPureModeEnabled
              ? controller.setPureModeEnabled
              : null,
        ),
      ],
    );
  }
}

class _SwitchCard extends StatelessWidget {
  const _SwitchCard({
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
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: SwitchListTile.adaptive(
        value: value,
        onChanged: onChanged == null ? null : (v) => onChanged!(v),
        title: Text(title),
        subtitle: Text(subtitle),
      ),
    );
  }
}
