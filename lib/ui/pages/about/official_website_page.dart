import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as mat show SelectableText;
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../providers/providers.dart';
import '../../utils/info_bar_helper.dart';
import '../../widgets/widgets.dart';

class OfficialWebsitePage extends ConsumerWidget {
  const OfficialWebsitePage({super.key});

  static const String _website = 'https://bayin.app';
  static const String _repository = 'https://github.com/';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Official website'),
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
              const BayinSectionHeader(title: 'LINKS'),
              _LinkCard(
                title: 'Website',
                value: _website,
              ),
              const SizedBox(height: 10),
              _LinkCard(
                title: 'Repository',
                value: _repository,
              ),
            ],
          ),
        ),
      ],
    );
  }
}


class _LinkCard extends ConsumerWidget {
  const _LinkCard({
    required this.title,
    required this.value,
  });

  final String title;
  final String value;

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
                  color: title == 'Website'
                      ? const Color(0xFF06B6D4)
                      : const Color(0xFF8B5CF6),
                  borderRadius: BorderRadius.circular(8),
                ),
                alignment: Alignment.center,
                child: Icon(
                  title == 'Website'
                      ? PhosphorIcons.globeHemisphereWest()
                      : PhosphorIcons.githubLogo(),
                  size: 15,
                  color: Colors.white,
                ),
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
          mat.SelectableText(
            value,
            style: TextStyle(color: tokens.textSecondary),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              FilledButton(
                onPressed: () => _openLink(context, ref, value),
                child: const Text('Open'),
              ),
              const SizedBox(width: 8),
              FilledButton(
                onPressed: () async {
                  await Clipboard.setData(ClipboardData(text: value));
                  if (!context.mounted) return;
                  showInfoMessage(context, 'Link copied to clipboard.');
                },
                child: const Text('Copy link'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _openLink(BuildContext context, WidgetRef ref, String raw) async {
    final uri = Uri.tryParse(raw);
    if (uri == null) {
      showInfoMessage(context, 'Invalid URL');
      return;
    }
    final opened = await launchUrl(uri, mode: LaunchMode.externalApplication);
    if (opened || !context.mounted) {
      return;
    }
    showInfoMessage(context, 'Failed to open URL.');
  }
}
