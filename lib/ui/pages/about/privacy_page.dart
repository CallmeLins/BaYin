import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class PrivacyPage extends ConsumerWidget {
  const PrivacyPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Privacy'),
          left: BayinGhostIconButton(
            icon: PhosphorIcons.caretLeft(),
            tooltip: 'Back',
            onTap: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/about');
              }
            },
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
              const BayinSectionHeader(title: 'PRIVACY'),
              const _PrivacyCard(
                title: 'Local-first storage',
                content:
                    'Library metadata and app settings are stored locally on your device by default.',
              ),
              const SizedBox(height: 10),
              const _PrivacyCard(
                title: 'Streaming credentials',
                content:
                    'Streaming server credentials are stored for your convenience and used only for requested connections.',
              ),
              const SizedBox(height: 10),
              const _PrivacyCard(
                title: 'No background telemetry',
                content:
                    'This app does not upload playback history or analytics to external services by default.',
              ),
            ],
          ),
        ),
      ],
    );
  }
}


class _PrivacyCard extends ConsumerWidget {
  const _PrivacyCard({
    required this.title,
    required this.content,
  });

  final String title;
  final String content;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final icon = switch (title) {
      'Local-first storage' => PhosphorIcons.hardDrives(),
      'Streaming credentials' => PhosphorIcons.key(),
      _ => PhosphorIcons.shieldCheck(),
    };
    final chipColor = switch (title) {
      'Local-first storage' => const Color(0xFF3B82F6),
      'Streaming credentials' => const Color(0xFFF59E0B),
      _ => const Color(0xFF10B981),
    };
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  color: chipColor,
                  borderRadius: BorderRadius.circular(8),
                ),
                alignment: Alignment.center,
                child: Icon(icon, size: 15, color: Colors.white),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  title,
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            content,
            style: TextStyle(color: tokens.textSecondary),
          ),
        ],
      ),
    );
  }
}
