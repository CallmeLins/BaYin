import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../widgets/widgets.dart';

class DonatePage extends ConsumerWidget {
  const DonatePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;

    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Donate'),
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
              // ── Hero ────────────────────────────────────────────────
              const SizedBox(height: 32),
              Center(
                child: Container(
                  width: 56,
                  height: 56,
                  decoration: BoxDecoration(
                    color: const Color(0xFFEF4444).withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(16),
                  ),
                  alignment: Alignment.center,
                  child: Icon(
                    PhosphorIcons.heart(),
                    size: 28,
                    color: const Color(0xFFEF4444),
                  ),
                ),
              ),
              const SizedBox(height: 16),
              Center(
                child: Text(
                  'Support BaYin',
                  style: TextStyle(
                    fontSize: 24,
                    fontWeight: FontWeight.w700,
                    color: brightness == Brightness.dark
                        ? Colors.white
                        : Colors.black,
                  ),
                ),
              ),
              const SizedBox(height: 8),
              Center(
                child: Text(
                  'Your support helps keep the project alive and improving.',
                  style: TextStyle(
                    fontSize: 14,
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.55)
                        : Colors.black.withValues(alpha: 0.50),
                  ),
                ),
              ),
              const SizedBox(height: 32),

              // ── Coffee Card ─────────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 44,
                        height: 44,
                        decoration: BoxDecoration(
                          color: const Color(0xFFFBBF24).withValues(
                            alpha: brightness == Brightness.dark ? 0.22 : 0.15,
                          ),
                          shape: BoxShape.circle,
                        ),
                        alignment: Alignment.center,
                        child: Icon(
                          PhosphorIcons.coffee(),
                          size: 22,
                          color: const Color(0xFFFBBF24),
                        ),
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text(
                              'Buy Me a Coffee',
                              style: TextStyle(
                                fontSize: 16,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 4),
                            Text(
                              'Scan the QR code below to donate via Alipay or WeChat Pay.',
                              style: TextStyle(
                                fontSize: 13,
                                color: brightness == Brightness.dark
                                    ? Colors.white.withValues(alpha: 0.55)
                                    : Colors.black.withValues(alpha: 0.50),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),

              const SizedBox(height: 16),

              // ── GitHub Star Card ────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 44,
                        height: 44,
                        decoration: BoxDecoration(
                          color: const Color(0xFF8B5CF6).withValues(
                            alpha: brightness == Brightness.dark ? 0.22 : 0.15,
                          ),
                          shape: BoxShape.circle,
                        ),
                        alignment: Alignment.center,
                        child: Icon(
                          PhosphorIcons.githubLogo(),
                          size: 22,
                          color: const Color(0xFF8B5CF6),
                        ),
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text(
                              'Star on GitHub',
                              style: TextStyle(
                                fontSize: 16,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 4),
                            Text(
                              'Give us a star on GitHub to show your support.',
                              style: TextStyle(
                                fontSize: 13,
                                color: brightness == Brightness.dark
                                    ? Colors.white.withValues(alpha: 0.55)
                                    : Colors.black.withValues(alpha: 0.50),
                              ),
                            ),
                            const SizedBox(height: 12),
                            GestureDetector(
                              onTap: () async {
                                final uri = Uri.parse('https://github.com/CallmeLins/BaYin');
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
                                      'Go to GitHub',
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

              const SizedBox(height: 16),

              // ── Share Card ──────────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 44,
                        height: 44,
                        decoration: BoxDecoration(
                          color: const Color(0xFF3B82F6).withValues(
                            alpha: brightness == Brightness.dark ? 0.22 : 0.15,
                          ),
                          shape: BoxShape.circle,
                        ),
                        alignment: Alignment.center,
                        child: Icon(
                          PhosphorIcons.shareNetwork(),
                          size: 22,
                          color: const Color(0xFF3B82F6),
                        ),
                      ),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text(
                              'Share with Friends',
                              style: TextStyle(
                                fontSize: 16,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 4),
                            Text(
                              'Spread the word about BaYin to help us grow.',
                              style: TextStyle(
                                fontSize: 13,
                                color: brightness == Brightness.dark
                                    ? Colors.white.withValues(alpha: 0.55)
                                    : Colors.black.withValues(alpha: 0.50),
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
              Center(
                child: Text(
                  'Thank you for your support! ❤️',
                  style: TextStyle(
                    fontSize: 13,
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.40)
                        : Colors.black.withValues(alpha: 0.35),
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
