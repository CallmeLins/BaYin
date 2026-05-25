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
              const BayinSectionHeader(title: 'LICENSES'),
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
          Row(
            children: [
              Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  color: const Color(0xFFF59E0B),
                  borderRadius: BorderRadius.circular(8),
                ),
                alignment: Alignment.center,
                child: Icon(PhosphorIcons.scales(), size: 15, color: Colors.white),
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
          const SizedBox(height: 8),
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
