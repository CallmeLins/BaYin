import 'package:flutter/material.dart';

class TermsPage extends StatelessWidget {
  const TermsPage({super.key});

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'Terms',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 12),
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
    );
  }
}

class _TermCard extends StatelessWidget {
  const _TermCard({
    required this.title,
    required this.content,
  });

  final String title;
  final String content;

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
          Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 6),
          Text(
            content,
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
        ],
      ),
    );
  }
}
