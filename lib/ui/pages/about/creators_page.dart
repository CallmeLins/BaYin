import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class CreatorsPage extends ConsumerWidget {
  const CreatorsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Creators'),
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
    final brightness = Theme.of(context).brightness;
    return BayinGlassCard(
      padding: const EdgeInsets.all(FlatSpacing.smPlus),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            name,
            style: FlatTypography.headingCompact(brightness),
          ),
          const SizedBox(height: FlatSpacing.xs),
          Text(
            role,
            style: FlatTypography.caption(brightness),
          ),
          const SizedBox(height: FlatSpacing.sm),
          Text(
            description,
            style: FlatTypography.bodySmall(brightness),
          ),
        ],
      ),
    );
  }
}
