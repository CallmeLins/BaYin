import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/cover_art.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — artists list.
class ArtistsPage extends ConsumerWidget {
  const ArtistsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncArtists = ref.watch(libraryArtistsProvider);
    return asyncArtists.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load artists\n$error'),
      ),
      data: (artists) {
        if (artists.isEmpty) {
          return Center(
            child: Padding(
              padding: const EdgeInsets.all(32),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    PhosphorIcons.microphone(),
                    size: 48,
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'No artists yet',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Scan music to populate the library.',
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
          );
        }
        return Column(
          children: [
            BayinPageHeader(
              title: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('Artists'),
                  const SizedBox(width: 8),
                  Text(
                    '${artists.length}',
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
              child: Container(
                margin: const EdgeInsets.fromLTRB(8, 0, 8, 0),
                decoration: BoxDecoration(
                  color: Theme.of(context).brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.04)
                      : Colors.black.withValues(alpha: 0.02),
                  borderRadius: BorderRadius.circular(12),
                  border: Border.all(
                    color: Theme.of(context).brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.07)
                        : Colors.black.withValues(alpha: 0.06),
                    width: 0.6,
                  ),
                ),
                child: ListView.builder(
                  padding: const EdgeInsets.fromLTRB(8, 6, 8, 8),
                  itemCount: artists.length,
                  itemExtent: 60,
                  itemBuilder: (context, index) {
                    return _ArtistRow(artist: artists[index]);
                  },
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _ArtistRow extends StatelessWidget {
  const _ArtistRow({required this.artist});

  final Artist artist;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        onTap: () => context.go('/artists/${Uri.encodeComponent(artist.id)}'),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
          child: Row(
            children: [
              CoverArt(
                width: 44,
                height: 44,
                coverHash: artist.coverHash,
                streamCoverUrl: artist.streamCoverUrl,
                size: CoverArtSize.small,
                shape: BoxShape.circle,
                placeholderIcon: PhosphorIcons.microphone(),
                placeholderIconSize: 20,
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      artist.name,
                      style: const TextStyle(
                        fontSize: 13.5,
                        fontWeight: FontWeight.w500,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      '${artist.songCount} songs',
                      style: TextStyle(
                        fontSize: 11,
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              Icon(
                PhosphorIcons.caretRight(),
                size: 14,
                color: scheme.onSurfaceVariant,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
