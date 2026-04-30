import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../widgets/widgets.dart';

class DonatePage extends StatelessWidget {
  const DonatePage({super.key});

  static const String _githubSponsors = 'https://github.com/sponsors';
  static const String _koFi = 'https://ko-fi.com/';

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Donate'),
          left: IconButton(
            tooltip: 'Back',
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/about');
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
              Text(
                'Support ongoing development and maintenance.',
                style: TextStyle(color: Theme.of(context).colorScheme.onSurfaceVariant),
              ),
              const SizedBox(height: 12),
              _DonateCard(
                title: 'GitHub Sponsors',
                value: _githubSponsors,
                icon: PhosphorIcons.heart(),
              ),
              const SizedBox(height: 10),
              _DonateCard(
                title: 'Ko-fi',
                value: _koFi,
                icon: PhosphorIcons.coffee(),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _DonateCard extends StatelessWidget {
  const _DonateCard({
    required this.title,
    required this.value,
    required this.icon,
  });

  final String title;
  final String value;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 16),
              const SizedBox(width: 6),
              Text(
                title,
                style: const TextStyle(fontWeight: FontWeight.w600),
              ),
            ],
          ),
          const SizedBox(height: 8),
          SelectableText(
            value,
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          const SizedBox(height: 8),
          FilledButton.tonalIcon(
            onPressed: () async {
              await Clipboard.setData(ClipboardData(text: value));
              if (!context.mounted) return;
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Link copied to clipboard.')),
              );
            },
            icon: const Icon(Icons.copy),
            label: const Text('Copy link'),
          ),
        ],
      ),
    );
  }
}
