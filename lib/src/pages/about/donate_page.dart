import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as mat show SelectableText;
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../utils/info_bar_helper.dart';
import '../../widgets/widgets.dart';

class DonatePage extends ConsumerWidget {
  const DonatePage({super.key});

  static const String _githubSponsors = 'https://github.com/sponsors';
  static const String _koFi = 'https://ko-fi.com/';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Donate'),
          left: IconButton(
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
                style: TextStyle(color: tokens.textSecondary),
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

class _DonateCard extends ConsumerWidget {
  const _DonateCard({
    required this.title,
    required this.value,
    required this.icon,
  });

  final String title;
  final String value;
  final IconData icon;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
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
          mat.SelectableText(
            value,
            style: TextStyle(color: tokens.textSecondary),
          ),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: () async {
              await Clipboard.setData(ClipboardData(text: value));
              if (!context.mounted) return;
              showInfoMessage(context, 'Link copied to clipboard.');
            },
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(PhosphorIcons.copy(), size: 18),
                const SizedBox(width: 6),
                const Text('Copy link'),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
