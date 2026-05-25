import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class AboutPage extends ConsumerWidget {
  const AboutPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;

    return Column(
      children: [
        BayinPageHeader(
          title: const Text('About BaYin'),
          right: BayinGhostIconButton(
            icon: PhosphorIcons.globeHemisphereWest(),
            tooltip: 'Official Website',
            onTap: () => context.go('/about/website'),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
              Container(
                padding: const EdgeInsets.all(14),
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
                    Text(
                      'BaYin 0.1.0',
                      style: FlatTypography.headingCompact(brightness),
                    ),
                    const SizedBox(height: 6),
                    Text(
                      'BaYin is a cross-platform music player built with Flutter + Rust FFI.',
                      style: FlatTypography.bodySmall(brightness).copyWith(
                        color: FlatColors.textSecondary(brightness),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 18),
              const BayinSectionHeader(title: 'INFORMATION'),
              _AboutNavCard(
                items: const [
                  _AboutNavItem(
                    icon: PhosphorIcons.users,
                    title: 'Creators',
                    route: '/about/creators',
                    chipColor: Color(0xFF3B82F6),
                  ),
                  _AboutNavItem(
                    icon: PhosphorIcons.fileText,
                    title: 'Terms',
                    route: '/about/terms',
                    chipColor: Color(0xFF8B5CF6),
                  ),
                  _AboutNavItem(
                    icon: PhosphorIcons.shieldCheck,
                    title: 'Privacy',
                    route: '/about/privacy',
                    chipColor: Color(0xFF10B981),
                  ),
                  _AboutNavItem(
                    icon: PhosphorIcons.scales,
                    title: 'Licenses',
                    route: '/about/licenses',
                    chipColor: Color(0xFFF59E0B),
                  ),
                ],
              ),
              const SizedBox(height: 18),
              const BayinSectionHeader(title: 'SUPPORT'),
              _AboutNavCard(
                items: const [
                  _AboutNavItem(
                    icon: PhosphorIcons.heart,
                    title: 'Donate',
                    route: '/about/donate',
                    chipColor: Color(0xFFEC4899),
                  ),
                  _AboutNavItem(
                    icon: PhosphorIcons.globeHemisphereWest,
                    title: 'Official website',
                    route: '/about/website',
                    chipColor: Color(0xFF06B6D4),
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

class _AboutNavItem {
  const _AboutNavItem({
    required this.icon,
    required this.title,
    required this.route,
    required this.chipColor,
  });

  final IconData Function([PhosphorIconsStyle]) icon;
  final String title;
  final String route;
  final Color chipColor;
}

class _AboutNavCard extends StatelessWidget {
  const _AboutNavCard({required this.items});

  final List<_AboutNavItem> items;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return BayinGlassGroup(
      child: Column(
        children: [
          for (var i = 0; i < items.length; i++) ...[
            _AboutNavRow(item: items[i]),
            if (i < items.length - 1)
              Padding(
                padding: const EdgeInsets.only(left: 56),
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
    );
  }
}

class _AboutNavRow extends StatefulWidget {
  const _AboutNavRow({required this.item});

  final _AboutNavItem item;

  @override
  State<_AboutNavRow> createState() => _AboutNavRowState();
}

class _AboutNavRowState extends State<_AboutNavRow> {
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
        onTap: () => context.go(widget.item.route),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          curve: Curves.easeOut,
          color: _hovering ? hoverBg : Colors.transparent,
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
          child: Row(
            children: [
              Container(
                width: 30,
                height: 30,
                decoration: BoxDecoration(
                  color: widget.item.chipColor,
                  borderRadius: BorderRadius.circular(8),
                ),
                alignment: Alignment.center,
                child: Icon(widget.item.icon(), size: 16, color: Colors.white),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  widget.item.title,
                  style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
                ),
              ),
              Icon(
                PhosphorIcons.caretRight(),
                size: 16,
                color: FlatColors.textSecondary(brightness),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
