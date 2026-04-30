import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class CreatorsPage extends ConsumerWidget {
  const CreatorsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Creators'),
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
              _CreatorCard(
                name: 'BaYin Team',
                role: 'Product and design',
                description: 'Owns product direction, UI/UX decisions and release planning.',
              ),
              const SizedBox(height: 10),
              _CreatorCard(
                name: 'Flutter Client',
                role: 'Cross-platform UI',
                description:
                    'Implements desktop/mobile user experience, state flow and interaction.',
              ),
              const SizedBox(height: 10),
              _CreatorCard(
                name: 'Rust Core',
                role: 'Audio engine and library backend',
                description:
                    'Provides playback, FFT, scanner/database and stream integrations via FFI.',
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _CreatorCard extends ConsumerWidget {
  const _CreatorCard({
    required this.name,
    required this.role,
    required this.description,
  });

  final String name;
  final String role;
  final String description;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return BayinGlassCard(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            name,
            style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 4),
          Text(
            role,
            style: TextStyle(
              fontSize: 12,
              color: tokens.textSecondary,
            ),
          ),
          const SizedBox(height: 8),
          Text(description),
        ],
      ),
    );
  }
}
