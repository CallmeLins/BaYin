import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class TermsPage extends ConsumerWidget {
  const TermsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Terms'),
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
              const _TermCard(
                title: 'Personal use',
                content:
                    'BaYin is provided for personal media playback and library management.',
              ),
              const SizedBox(height: 10),
              const _TermCard(
                title: 'Content responsibility',
                content:
                    'You are responsible for permissions and rights of all local and streamed content you play.',
              ),
              const SizedBox(height: 10),
              const _TermCard(
                title: 'No warranty',
                content:
                    'The software is provided "as is" without warranties of any kind, express or implied.',
              ),
            ],
          ),
        ),
      ],
    );
  }
}


class _TermCard extends ConsumerWidget {
  const _TermCard({
    required this.title,
    required this.content,
  });

  final String title;
  final String content;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w600),
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
