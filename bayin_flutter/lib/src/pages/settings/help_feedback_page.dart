import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/providers.dart';

class HelpFeedbackPage extends ConsumerWidget {
  const HelpFeedbackPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final platform = ref.watch(platformProvider);
    final app = ref.watch(appSettingsProvider);
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'Help & Feedback',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 12),
        _InfoCard(
          title: 'Common actions',
          children: const [
            '1. Use Scan page to import local audio files.',
            '2. Configure stream servers from Stream settings.',
            '3. If playback fails, test source URL/network first.',
          ],
        ),
        const SizedBox(height: 10),
        _InfoCard(
          title: 'Diagnostics',
          children: [
            'OS: ${platform.operatingSystem} ${platform.operatingSystemVersion}',
            'Visualizer: ${app.visualizerEnabled ? 'on' : 'off'}',
            'EQ: ${app.eqEnabled ? 'on' : 'off'}',
          ],
        ),
        const SizedBox(height: 10),
        FilledButton.tonalIcon(
          onPressed: () async {
            final text = [
              'BaYin diagnostics',
              'os=${platform.operatingSystem}',
              'osVersion=${platform.operatingSystemVersion}',
              'visualizer=${app.visualizerEnabled}',
              'eq=${app.eqEnabled}',
            ].join('\n');
            await Clipboard.setData(ClipboardData(text: text));
            if (!context.mounted) return;
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(content: Text('Diagnostics copied to clipboard.')),
            );
          },
          icon: const Icon(Icons.copy),
          label: const Text('Copy diagnostics'),
        ),
      ],
    );
  }
}

class _InfoCard extends StatelessWidget {
  const _InfoCard({
    required this.title,
    required this.children,
  });

  final String title;
  final List<String> children;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
          const SizedBox(height: 8),
          for (final line in children)
            Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: Text(
                line,
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
            ),
        ],
      ),
    );
  }
}
