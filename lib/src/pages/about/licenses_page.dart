import 'package:flutter/material.dart';

class LicensesPage extends StatelessWidget {
  const LicensesPage({super.key});

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'Licenses',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 12),
        _LicenseCard(
          title: 'Open source components',
          content:
              'BaYin uses Flutter, Rust and multiple open-source libraries. View the full license list below.',
          actionLabel: 'View full licenses',
          onTap: () {
            showLicensePage(
              context: context,
              applicationName: 'BaYin',
              applicationVersion: '0.1.0',
            );
          },
        ),
      ],
    );
  }
}

class _LicenseCard extends StatelessWidget {
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
          const SizedBox(height: 10),
          FilledButton.tonal(
            onPressed: onTap,
            child: Text(actionLabel),
          ),
        ],
      ),
    );
  }
}
