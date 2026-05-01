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
          right: Tooltip(
            message: 'Official Website',
            child: IconButton(
              onPressed: () => context.go('/about/website'),
              icon: Icon(
                PhosphorIcons.globeHemisphereWest(),
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
              const _IntroCard(
                description:
                    'BaYin is a cross-platform music player built with Flutter + Rust FFI.',
              ),
              const SizedBox(height: 24),
              _GroupCard(
                title: 'INFORMATION',
                items: [
                  _AboutItem(
                    icon: PhosphorIcons.users(),
                    title: 'Creators',
                    subtitle: 'Team members and responsibilities',
                    route: '/about/creators',
                  ),
                  _AboutItem(
                    icon: PhosphorIcons.fileText(),
                    title: 'Terms',
                    subtitle: 'Terms of use and limitations',
                    route: '/about/terms',
                  ),
                  _AboutItem(
                    icon: PhosphorIcons.shieldCheck(),
                    title: 'Privacy',
                    subtitle: 'Data handling and privacy policy',
                    route: '/about/privacy',
                  ),
                  _AboutItem(
                    icon: PhosphorIcons.scales(),
                    title: 'Licenses',
                    subtitle: 'Open-source licenses and acknowledgements',
                    route: '/about/licenses',
                  ),
                ],
              ),
              const SizedBox(height: 24),
              _GroupCard(
                title: 'SUPPORT',
                items: [
                  _AboutItem(
                    icon: PhosphorIcons.heart(),
                    title: 'Donate',
                    subtitle: 'Support project development',
                    route: '/about/donate',
                  ),
                  _AboutItem(
                    icon: PhosphorIcons.globeHemisphereWest(),
                    title: 'Official website',
                    subtitle: 'Project links and release channels',
                    route: '/about/website',
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

class _IntroCard extends ConsumerWidget {
  const _IntroCard({required this.description});

  final String description;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Container(
      padding: const EdgeInsets.all(FlatSpacing.md),
      decoration: BoxDecoration(
        color: FlatColors.muted(brightness),
        borderRadius: BorderRadius.circular(FlatRadius.md),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'BaYin 0.1.0',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: FlatColors.foreground(brightness),
            ),
          ),
          const SizedBox(height: 6),
          Text(
            description,
            style: TextStyle(
              color: FlatColors.textSecondary(brightness),
            ),
          ),
        ],
      ),
    );
  }
}

class _GroupCard extends ConsumerWidget {
  const _GroupCard({
    required this.title,
    required this.items,
  });

  final String title;
  final List<_AboutItem> items;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xs + 2),
          child: Text(
            title,
            style: FlatTypography.label(brightness),
          ),
        ),
        Container(
          decoration: BoxDecoration(
            color: FlatColors.background(brightness),
            borderRadius: BorderRadius.circular(FlatRadius.md),
            border: Border.all(
              color: FlatColors.border(brightness),
              width: FlatBorder.structural,
            ),
          ),
          child: Column(
            children: [
              for (var i = 0; i < items.length; i++) ...[
                if (i > 0)
                  Divider(
                    height: FlatBorder.structural,
                    indent: 48,
                    color: FlatColors.border(brightness),
                  ),
                _AboutTile(item: items[i]),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

class _AboutItem {
  const _AboutItem({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.route,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final String route;
}

class _AboutTile extends ConsumerWidget {
  const _AboutTile({required this.item});

  final _AboutItem item;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return ListTile(
      onTap: () => context.go(item.route),
      leading: Icon(
        item.icon,
        size: 20,
        color: FlatColors.primary(brightness),
      ),
      title: Text(
        item.title,
        style: FlatTypography.bodySmall(brightness).copyWith(
          fontWeight: FontWeight.w500,
        ),
      ),
      subtitle: Text(
        item.subtitle,
        style: FlatTypography.caption(brightness),
      ),
    );
  }
}
