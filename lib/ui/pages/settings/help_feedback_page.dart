import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../widgets/widgets.dart';

class HelpFeedbackPage extends ConsumerWidget {
  const HelpFeedbackPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;

    final helpItems = [
      _HelpItem(
        icon: PhosphorIcons.book(),
        title: 'User Guide',
        description: 'Read the project README and documentation',
        url: 'https://github.com/CallmeLins/BaYin#readme',
        color: const Color(0xFF3B82F6),
      ),
      _HelpItem(
        icon: PhosphorIcons.bug(),
        title: 'Report Issue',
        description: 'Found a bug? Let us know on GitHub',
        url: 'https://github.com/CallmeLins/BaYin/issues',
        color: const Color(0xFFEF4444),
      ),
      _HelpItem(
        icon: PhosphorIcons.lightbulb(),
        title: 'Feature Request',
        description: 'Suggest new features or improvements',
        url: 'https://github.com/CallmeLins/BaYin/issues',
        color: const Color(0xFFF59E0B),
      ),
      _HelpItem(
        icon: PhosphorIcons.chatsCircle(),
        title: 'Community',
        description: 'Join discussions with other users',
        url: 'https://github.com/CallmeLins/BaYin/discussions',
        color: const Color(0xFF8B5CF6),
      ),
      _HelpItem(
        icon: PhosphorIcons.envelope(),
        title: 'Contact Us',
        description: 'Send us an email directly',
        url: 'mailto:zhao5638@gmail.com',
        color: const Color(0xFF10B981),
      ),
    ];

    final faqItems = [
      _FaqItem(
        question: 'How to import music?',
        answer: 'Go to the Scan page, add a local folder or configure a stream server, then tap "Start Scan".',
      ),
      _FaqItem(
        question: 'What formats are supported?',
        answer: 'MP3, FLAC, WAV, AAC, M4A, OGG, AIFF, OPUS and more are supported via the Rust audio engine.',
      ),
      _FaqItem(
        question: 'How to create playlists?',
        answer: 'Go to Playlists page and tap "New". You can also add songs to playlists from the song menu.',
      ),
    ];

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
              const SizedBox(height: 16),

              // ── Resources Section ────────────────────────────────────
              const BayinSectionHeader(title: 'RESOURCES'),
              BayinGlassGroup(
                child: Column(
                  children: [
                    for (var i = 0; i < helpItems.length; i++) ...[
                      _HelpRow(
                        item: helpItems[i],
                        onTap: () async {
                          final uri = Uri.tryParse(helpItems[i].url);
                          if (uri != null) {
                            await launchUrl(uri, mode: LaunchMode.externalApplication);
                          }
                        },
                      ),
                      if (i < helpItems.length - 1)
                        Padding(
                          padding: const EdgeInsets.only(left: 60),
                          child: Divider(
                            height: 1,
                            thickness: 1,
                            color: brightness == Brightness.dark
                                ? Colors.white.withValues(alpha: 0.06)
                                : Colors.black.withValues(alpha: 0.06),
                          ),
                        ),
                    ],
                  ],
                ),
              ),

              const SizedBox(height: 24),

              // ── FAQ Section ──────────────────────────────────────────
              const BayinSectionHeader(title: 'FAQ'),
              for (final faq in faqItems)
                Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: BayinGlassGroup(
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            faq.question,
                            style: const TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                          const SizedBox(height: 8),
                          Text(
                            faq.answer,
                            style: TextStyle(
                              fontSize: 13,
                              color: brightness == Brightness.dark
                                  ? Colors.white.withValues(alpha: 0.55)
                                  : Colors.black.withValues(alpha: 0.50),
                              height: 1.5,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

class _HelpItem {
  const _HelpItem({
    required this.icon,
    required this.title,
    required this.description,
    required this.url,
    required this.color,
  });

  final IconData icon;
  final String title;
  final String description;
  final String url;
  final Color color;
}

class _FaqItem {
  const _FaqItem({
    required this.question,
    required this.answer,
  });

  final String question;
  final String answer;
}

class _HelpRow extends StatefulWidget {
  const _HelpRow({
    required this.item,
    required this.onTap,
  });

  final _HelpItem item;
  final VoidCallback onTap;

  @override
  State<_HelpRow> createState() => _HelpRowState();
}

class _HelpRowState extends State<_HelpRow> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final hoverBg = brightness == Brightness.dark
        ? Colors.white.withValues(alpha: 0.05)
        : Colors.black.withValues(alpha: 0.04);

    return MouseRegion(
      onEnter: (_) => setState(() => _hovering = true),
      onExit: (_) => setState(() => _hovering = false),
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          color: _hovering ? hoverBg : Colors.transparent,
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
          child: Row(
            children: [
              Container(
                width: 30,
                height: 30,
                decoration: BoxDecoration(
                  color: widget.item.color,
                  borderRadius: BorderRadius.circular(8),
                ),
                alignment: Alignment.center,
                child: Icon(widget.item.icon, size: 16, color: Colors.white),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.item.title,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    Text(
                      widget.item.description,
                      style: TextStyle(
                        fontSize: 12,
                        color: brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.45)
                            : Colors.black.withValues(alpha: 0.40),
                      ),
                    ),
                  ],
                ),
              ),
              Icon(
                PhosphorIcons.caretRight(),
                size: 16,
                color: brightness == Brightness.dark
                    ? Colors.white.withValues(alpha: 0.35)
                    : Colors.black.withValues(alpha: 0.30),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
