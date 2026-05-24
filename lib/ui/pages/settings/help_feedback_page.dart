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
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Help & Feedback'),
          left: BayinGhostIconButton(
            icon: PhosphorIcons.caretLeft(),
            tooltip: 'Back',
            onTap: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
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
          child: Row(
            children: [
              Text(
                title,
                style: FlatTypography.label(brightness),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Divider(
                  height: 1,
                  thickness: 1,
                  color: brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.10)
                      : Colors.black.withValues(alpha: 0.06),
                ),
              ),
            ],
          ),
        ),
        Container(
          padding: const EdgeInsets.all(FlatSpacing.md),
          decoration: BoxDecoration(
            color: brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.04)
                : Colors.black.withValues(alpha: 0.02),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(
              color: brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.08)
                  : Colors.black.withValues(alpha: 0.06),
              width: 0.8,
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
