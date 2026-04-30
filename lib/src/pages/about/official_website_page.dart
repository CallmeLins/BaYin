import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../widgets/widgets.dart';

class OfficialWebsitePage extends StatelessWidget {
  const OfficialWebsitePage({super.key});

  static const String _website = 'https://bayin.app';
  static const String _repository = 'https://github.com/';

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Official website'),
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
              _LinkCard(
                title: 'Website',
                value: _website,
              ),
              const SizedBox(height: 10),
              _LinkCard(
                title: 'Repository',
                value: _repository,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _LinkCard extends StatelessWidget {
  const _LinkCard({
    required this.title,
    required this.value,
  });

  final String title;
  final String value;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
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
          SelectableText(
            value,
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              FilledButton(
                onPressed: () => _openLink(context, value),
                child: const Text('Open'),
              ),
              const SizedBox(width: 8),
              FilledButton.tonal(
                onPressed: () async {
                  await Clipboard.setData(ClipboardData(text: value));
                  if (!context.mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(content: Text('Link copied to clipboard.')),
                  );
                },
                child: const Text('Copy link'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _openLink(BuildContext context, String raw) async {
    final uri = Uri.tryParse(raw);
    if (uri == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Invalid URL')),
      );
      return;
    }
    final opened = await launchUrl(uri, mode: LaunchMode.externalApplication);
    if (opened || !context.mounted) {
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Failed to open URL.')),
    );
  }
}
