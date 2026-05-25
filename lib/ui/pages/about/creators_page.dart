import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../widgets/widgets.dart';

class CreatorsPage extends ConsumerWidget {
  const CreatorsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;

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
              const SizedBox(height: 16),

              // ── Creator Card ────────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 80,
                        height: 80,
                        decoration: BoxDecoration(
                          color: brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.10)
                              : Colors.black.withValues(alpha: 0.05),
                          shape: BoxShape.circle,
                        ),
                        alignment: Alignment.center,
                        child: Icon(
                          PhosphorIcons.code(),
                          size: 36,
                          color: brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.80)
                              : Colors.black.withValues(alpha: 0.70),
                        ),
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text(
                              'Lins',
                              style: TextStyle(
                                fontSize: 20,
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            const SizedBox(height: 4),
                            const Text(
                              'Lead Developer',
                              style: TextStyle(
                                fontSize: 14,
                                fontWeight: FontWeight.w500,
                                color: Color(0xFF3B82F6),
                              ),
                            ),
                            const SizedBox(height: 8),
                            Text(
                              'Builds the Flutter + Rust cross-platform music player.',
                              style: TextStyle(
                                fontSize: 13,
                                color: brightness == Brightness.dark
                                    ? Colors.white.withValues(alpha: 0.55)
                                    : Colors.black.withValues(alpha: 0.50),
                                height: 1.5,
                              ),
                            ),
                            const SizedBox(height: 12),
                            GestureDetector(
                              onTap: () async {
                                final uri = Uri.parse('https://github.com/CallmeLins');
                                await launchUrl(uri, mode: LaunchMode.externalApplication);
                              },
                              child: Container(
                                padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                                decoration: BoxDecoration(
                                  color: brightness == Brightness.dark
                                      ? Colors.white
                                      : Colors.black,
                                  borderRadius: BorderRadius.circular(8),
                                ),
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    Icon(
                                      PhosphorIcons.githubLogo(),
                                      size: 16,
                                      color: brightness == Brightness.dark
                                          ? Colors.black
                                          : Colors.white,
                                    ),
                                    const SizedBox(width: 6),
                                    Text(
                                      'GitHub Profile',
                                      style: TextStyle(
                                        fontSize: 13,
                                        fontWeight: FontWeight.w500,
                                        color: brightness == Brightness.dark
                                            ? Colors.black
                                            : Colors.white,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),

              const SizedBox(height: 24),

              // ── Thanks Section ──────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        'Special Thanks',
                        style: TextStyle(
                          fontSize: 16,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      const SizedBox(height: 8),
                      Text(
                        'Thanks to all contributors and the open source community for making this project possible.',
                        style: TextStyle(
                          fontSize: 13,
                          color: brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.55)
                              : Colors.black.withValues(alpha: 0.50),
                          height: 1.5,
                        ),
                      ),
                      const SizedBox(height: 12),
                      GestureDetector(
                        onTap: () async {
                          final uri = Uri.parse('https://github.com/CallmeLins/BaYin/graphs/contributors');
                          await launchUrl(uri, mode: LaunchMode.externalApplication);
                        },
                        child: const Text(
                          'View Contributors →',
                          style: TextStyle(
                            fontSize: 13,
                            fontWeight: FontWeight.w500,
                            color: Color(0xFF3B82F6),
                          ),
                        ),
                      ),
                    ],
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
