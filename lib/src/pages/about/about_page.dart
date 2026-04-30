import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../widgets/widgets.dart';

class AboutPage extends StatelessWidget {
  const AboutPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('About BaYin'),
          right: IconButton(
            tooltip: 'Website',
            onPressed: () => context.go('/about/website'),
            icon: Icon(PhosphorIcons.globeHemisphereWest()),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 2),
              const _IntroCard(
                description:
                    'BaYin is a cross-platform music player built with Flutter + Rust FFI.',
              ),
              const SizedBox(height: 12),
              _GroupCard(
                title: 'Information',
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
              const SizedBox(height: 12),
              _GroupCard(
                title: 'Support',
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

class _IntroCard extends StatelessWidget {
  const _IntroCard({required this.description});

  final String description;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'BaYin 0.1.0',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 6),
          Text(
            description,
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
        ],
      ),
    );
  }
}

class _GroupCard extends StatelessWidget {
  const _GroupCard({
    required this.title,
    required this.items,
  });

  final String title;
  final List<_AboutItem> items;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 6),
            child: Row(
              children: [
                Text(
                  title,
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
              ],
            ),
          ),
          for (var i = 0; i < items.length; i++) ...[
            if (i > 0) Divider(height: 1, color: scheme.outlineVariant),
            _AboutTile(item: items[i]),
          ],
        ],
      ),
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

class _AboutTile extends StatelessWidget {
  const _AboutTile({required this.item});

  final _AboutItem item;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: () => context.go(item.route),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
          child: Row(
            children: [
              Icon(item.icon, size: 18),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(item.title),
                    const SizedBox(height: 2),
                    Text(
                      item.subtitle,
                      style: TextStyle(
                        fontSize: 12,
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              Icon(PhosphorIcons.caretRight(), size: 16),
            ],
          ),
        ),
      ),
    );
  }
}
