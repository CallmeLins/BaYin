import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class HelpFeedbackPage extends ConsumerWidget {
  const HelpFeedbackPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final platform = ref.watch(platformProvider);
    final app = ref.watch(appSettingsProvider);
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Help & Feedback'),
          left: IconButton(
            tooltip: 'Back',
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
          ),
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
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
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
