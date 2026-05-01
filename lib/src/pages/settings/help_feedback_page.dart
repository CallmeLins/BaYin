import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
import '../../utils/info_bar_helper.dart';
import '../../widgets/widgets.dart';

class HelpFeedbackPage extends ConsumerWidget {
  const HelpFeedbackPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final platform = ref.watch(platformProvider);
    final app = ref.watch(appSettingsProvider);
    final brightness = Theme.of(context).brightness;
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Help & Feedback'),
          left: Tooltip(
            message: 'Back',
            child: IconButton(
              onPressed: () {
                if (context.canPop()) {
                  context.pop();
                } else {
                  context.go('/settings');
                }
              },
              icon: Icon(
                PhosphorIcons.caretLeft(),
                size: 20,
                color: FlatColors.textSecondary(brightness),
              ),
            ),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 20),
            children: [
              const SizedBox(height: 12),
              _InfoCard(
                title: 'COMMON ACTIONS',
                children: const [
                  '1. Use Scan page to import local audio files.',
                  '2. Configure stream servers from Stream settings.',
                  '3. If playback fails, test source URL/network first.',
                ],
              ),
              const SizedBox(height: FlatSpacing.lg),
              _InfoCard(
                title: 'DIAGNOSTICS',
                children: [
                  'OS: ${platform.operatingSystem} ${platform.operatingSystemVersion}',
                  'Visualizer: ${app.visualizerEnabled ? 'on' : 'off'}',
                  'EQ: ${app.eqEnabled ? 'on' : 'off'}',
                ],
              ),
              const SizedBox(height: FlatSpacing.lg),
              Row(
                children: [
                  FilledButton.icon(
                    onPressed: () async {
                      final text = [
                        'BaYin diagnostics',
                        'os=${platform.operatingSystem}',
                        'osVersion=${platform.operatingSystemVersion}',
                        'visualizer=${app.visualizerEnabled}',
                        'eq=${app.eqEnabled}',
                      ].join('\n');
                      await Clipboard.setData(ClipboardData(text: text));
                      if (!context.mounted) return;
                      showInfoMessage(context, 'Diagnostics copied to clipboard.');
                    },
                    icon: Icon(PhosphorIcons.copy(), size: 18),
                    label: const Text('Copy diagnostics'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _InfoCard extends ConsumerWidget {
  const _InfoCard({
    required this.title,
    required this.children,
  });

  final String title;
  final List<String> children;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xs + 2,
          ),
          child: Text(
            title,
            style: FlatTypography.label(brightness),
          ),
        ),
        Container(
          padding: const EdgeInsets.all(FlatSpacing.md),
          decoration: BoxDecoration(
            color: FlatColors.background(brightness),
            borderRadius: BorderRadius.circular(FlatRadius.md),
            border: Border.all(
              color: FlatColors.border(brightness),
              width: FlatBorder.structural,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (final line in children)
                Padding(
                  padding: const EdgeInsets.only(bottom: FlatSpacing.xs + 2),
                  child: Text(
                    line,
                    style: FlatTypography.bodySmall(brightness),
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
