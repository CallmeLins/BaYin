import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../providers/providers.dart';

class PlaceholderPage extends ConsumerWidget {
  const PlaceholderPage({
    super.key,
    required this.title,
    this.phase,
    this.note,
  });

  final String title;
  final String? phase;
  final String? note;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final routerState = GoRouterState.of(context);
    final path = routerState.matchedLocation;
    final params = routerState.pathParameters;
    final tokens = ref.watch(bayinTokensProvider);

    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              title,
              style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w600),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            Text(
              'Route: $path',
              style: TextStyle(fontSize: 12, color: tokens.textSecondary),
            ),
            if (params.isNotEmpty) ...[
              const SizedBox(height: 4),
              Text(
                'Params: ${params.entries.map((e) => '${e.key}=${e.value}').join(', ')}',
                style: TextStyle(fontSize: 12, color: tokens.textSecondary),
              ),
            ],
            if (phase != null) ...[
              const SizedBox(height: 12),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                decoration: BoxDecoration(
                  color: tokens.isDark
                      ? Colors.white.withValues(alpha: 0.08)
                      : Colors.black.withValues(alpha: 0.06),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  phase!,
                  style: TextStyle(
                    fontSize: 12,
                    color: tokens.textPrimary,
                  ),
                ),
              ),
            ],
            if (note != null) ...[
              const SizedBox(height: 12),
              Text(
                note!,
                style: TextStyle(fontSize: 14, color: tokens.textSecondary),
                textAlign: TextAlign.center,
              ),
            ],
          ],
        ),
      ),
    );
  }
}
