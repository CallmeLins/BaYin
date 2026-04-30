import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/cover_art.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — local album grid.
class AlbumsPage extends ConsumerWidget {
  const AlbumsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncAlbums = ref.watch(libraryAlbumsProvider);
    return asyncAlbums.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load albums\n$error'),
      ),
      data: (albums) {
        if (albums.isEmpty) {
          return _EmptyState(
            icon: PhosphorIcons.vinylRecord(),
            title: 'No albums yet',
            subtitle: 'Scan music to populate the library.',
          );
        }
        final isDark = Theme.of(context).brightness == Brightness.dark;
        return Column(
          children: [
            BayinPageHeader(
              title: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('Albums'),
                  const SizedBox(width: 8),
                  Text(
                    '${albums.length}',
                    style: TextStyle(
                      fontSize: 13,
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
              right: IconButton(
                tooltip: 'Search',
                onPressed: () => context.go('/search'),
                icon: Icon(PhosphorIcons.magnifyingGlass()),
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 0, 8, 0),
                child: Container(
                  padding: const EdgeInsets.fromLTRB(10, 12, 10, 8),
                  decoration: BoxDecoration(
                    color: isDark
                        ? Colors.white.withValues(alpha: 0.04)
                        : Colors.black.withValues(alpha: 0.02),
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(
                      color: isDark
                          ? Colors.white.withValues(alpha: 0.07)
                          : Colors.black.withValues(alpha: 0.06),
                      width: 0.6,
                    ),
                  ),
                  child: GridView.builder(
                    gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                      maxCrossAxisExtent: 180,
                      mainAxisSpacing: 16,
                      crossAxisSpacing: 16,
                      childAspectRatio: 0.78,
                    ),
                    itemCount: albums.length,
                    itemBuilder: (context, index) {
                      return _AlbumCard(album: albums[index]);
                    },
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _AlbumCard extends StatelessWidget {
  const _AlbumCard({required this.album});

  final Album album;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(10),
        onTap: () => context.go('/albums/${Uri.encodeComponent(album.id)}'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            AspectRatio(
              aspectRatio: 1,
              child: CoverArt(
                width: double.infinity,
                height: double.infinity,
                coverHash: album.coverHash,
                streamCoverUrl: album.streamCoverUrl,
                size: CoverArtSize.mid,
                borderRadius: BorderRadius.circular(8),
                placeholderIcon: PhosphorIcons.vinylRecord(),
                placeholderIconSize: 40,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              album.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: 2),
            Text(
              album.artist,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: 11,
                color: scheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              icon,
              size: 48,
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
            const SizedBox(height: 12),
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 6),
            Text(
              subtitle,
              style: TextStyle(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
