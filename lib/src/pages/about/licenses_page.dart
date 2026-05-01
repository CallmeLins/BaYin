import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as mat show showLicensePage;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class LicensesPage extends ConsumerWidget {
  const LicensesPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Licenses'),
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
              _LicenseCard(
                title: 'Open source components',
                content:
                    'BaYin uses Flutter, Rust and multiple open-source libraries. View the full license list below.',
                actionLabel: 'View full licenses',
                onTap: () {
                  mat.showLicensePage(
                    context: context,
                    applicationName: 'BaYin',
                    applicationVersion: '0.1.0',
                  );
                },
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _LicenseCard extends ConsumerWidget {
  const _LicenseCard({
    required this.title,
    required this.content,
    required this.actionLabel,
    required this.onTap,
  });

  final String title;
  final String content;
  final String actionLabel;
  final VoidCallback onTap;

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
          const SizedBox(height: 10),
          FilledButton(
            onPressed: onTap,
            child: Text(actionLabel),
          ),
        ],
      ),
    );
  }
}
