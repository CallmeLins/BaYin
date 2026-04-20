import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

/// Phase 1 placeholder page.
///
/// Shows the target page name, the matched route path, and any path
/// parameters. Replaced per-page in Phase 3+ with the real implementation.
/// Rendered inside RootShell — do not add its own Scaffold.
class PlaceholderPage extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final routerState = GoRouterState.of(context);
    final path = routerState.matchedLocation;
    final params = routerState.pathParameters;

    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              title,
              style: Theme.of(context).textTheme.headlineSmall,
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            Text(
              'Route: $path',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            if (params.isNotEmpty) ...[
              const SizedBox(height: 4),
              Text(
                'Params: ${params.entries.map((e) => '${e.key}=${e.value}').join(', ')}',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
            if (phase != null) ...[
              const SizedBox(height: 12),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.secondaryContainer,
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  phase!,
                  style: TextStyle(
                    fontSize: 12,
                    color: Theme.of(context).colorScheme.onSecondaryContainer,
                  ),
                ),
              ),
            ],
            if (note != null) ...[
              const SizedBox(height: 12),
              Text(
                note!,
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
            ],
          ],
        ),
      ),
    );
  }
}
