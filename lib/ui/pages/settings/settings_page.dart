import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../i18n/strings.g.dart';
import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final t = context.t;

    return Column(
      children: [
        BayinPageHeader(
          title: Text(t.nav.settings),
          right: BayinGhostIconButton(
            icon: PhosphorIcons.info(),
            tooltip: 'About',
            onTap: () => context.go('/about'),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 10),
              _SettingsNavCard(
                items: [
                  _NavItemData(
                    icon: PhosphorIcons.sparkle(),
                    title: 'BaYin Pro',
                    route: '/settings/pro',
                    chipColor: const Color(0xFFEAB308),
                  ),
                  _NavItemData(
                    icon: PhosphorIcons.palette(),
                    title: 'User interface',
                    route: '/settings/interface',
                    chipColor: const Color(0xFF3B82F6),
                  ),
                  _NavItemData(
                    icon: PhosphorIcons.textAa(),
                    title: 'Lyrics',
                    route: '/settings/lyrics',
                    chipColor: const Color(0xFFEC4899),
                  ),
                  _NavItemData(
                    icon: PhosphorIcons.sliders(),
                    title: 'Equalizer',
                    route: '/settings/equalizer',
                    chipColor: const Color(0xFF06B6D4),
                  ),
                  _NavItemData(
                    icon: PhosphorIcons.chatTeardropText(),
                    title: 'Help & feedback',
                    route: '/settings/help',
                    chipColor: const Color(0xFFA855F7),
                  ),
                  _NavItemData(
                    icon: PhosphorIcons.downloadSimple(),
                    title: 'Update software',
                    route: '/settings/update',
                    chipColor: const Color(0xFF22C55E),
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


class _SettingsNavCard extends StatelessWidget {
  const _SettingsNavCard({required this.items});

  final List<_NavItemData> items;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
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
        children: [
          for (var i = 0; i < items.length; i++) ...[
            _SettingsNavRow(item: items[i]),
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

class _NavItemData {
  const _NavItemData({
    required this.icon,
    required this.title,
    required this.route,
    required this.chipColor,
  });

  final IconData icon;
  final String title;
  final String route;
  final Color chipColor;
}

class _SettingsNavRow extends StatefulWidget {
  const _SettingsNavRow({required this.item});

  final _NavItemData item;

  @override
  State<_SettingsNavRow> createState() => _SettingsNavRowState();
}

class _SettingsNavRowState extends State<_SettingsNavRow> {
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
                child: Icon(widget.item.icon, size: 16, color: Colors.white),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  widget.item.title,
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                  ),
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
