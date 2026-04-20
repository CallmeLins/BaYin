import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — one artist's catalogue. Shows all songs + album chips grouped.
class ArtistDetailPage extends ConsumerWidget {
  const ArtistDetailPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final artistId = Uri.decodeComponent(
      GoRouterState.of(context).pathParameters['artistId'] ?? '',
    );
    final asyncArtists = ref.watch(libraryArtistsProvider);
    final asyncSongs = ref.watch(librarySongsProvider);

    return asyncArtists.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load artist\n$error'),
      ),
      data: (artists) {
        final Artist? artist =
            artists.where((a) => a.id == artistId).cast<Artist?>().firstOrNull;
        if (artist == null) {
          return _ArtistNotFound(id: artistId);
        }
        return asyncSongs.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(
            child: SelectableText('Failed to load songs\n$error'),
          ),
          data: (songs) {
            final mine =
                songs.where((s) => s.artist == artist.name).toList(growable: false);
            final albumNames = <String>{
              for (final s in mine) s.album,
            }.toList()
              ..sort();
            return Column(
              children: [
                _ArtistHeader(
                  artist: artist,
                  songCount: mine.length,
                  albumCount: albumNames.length,
                ),
                if (albumNames.isNotEmpty)
                  _AlbumChips(albumNames: albumNames),
                const Divider(height: 1),
                Expanded(
                  child: mine.isEmpty
                      ? Center(
                          child: Text(
                            'No songs found for "${artist.name}".',
                            style: TextStyle(
                              color: Theme.of(context).colorScheme.onSurfaceVariant,
                            ),
                          ),
                        )
                      : SongList(
                          songs: mine,
                          showIndex: true,
                          onTap: (song) => ref
                              .read(playerControllerProvider.notifier)
                              .playQueue(
                                mine,
                                startIndex: mine.indexOf(song),
                              ),
                          onLongPress: (song) =>
                              SongMenu.show(context, song: song),
                        ),
                ),
              ],
            );
          },
        );
      },
    );
  }
}

class _ArtistHeader extends StatelessWidget {
  const _ArtistHeader({
    required this.artist,
    required this.songCount,
    required this.albumCount,
  });

  final Artist artist;
  final int songCount;
  final int albumCount;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 8),
      child: Row(
        children: [
          IconButton(
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/artists');
              }
            },
            icon: Icon(PhosphorIcons.arrowLeft()),
            tooltip: 'Back',
          ),
          const SizedBox(width: 4),
          Container(
            width: 64,
            height: 64,
            decoration: BoxDecoration(
              color: scheme.surfaceContainerHighest,
              shape: BoxShape.circle,
            ),
            alignment: Alignment.center,
            child: Icon(
              PhosphorIcons.microphone(),
              size: 30,
              color: scheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  artist.name,
                  style: Theme.of(context).textTheme.titleLarge,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 4),
                Text(
                  '$albumCount albums · $songCount tracks',
                  style: TextStyle(color: scheme.onSurfaceVariant),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _AlbumChips extends StatelessWidget {
  const _AlbumChips({required this.albumNames});

  final List<String> albumNames;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 44,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        itemCount: albumNames.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, index) {
          final name = albumNames[index];
          return Chip(
            label: Text(name, overflow: TextOverflow.ellipsis),
            avatar: Icon(PhosphorIcons.vinylRecord(), size: 14),
          );
        },
      ),
    );
  }
}

class _ArtistNotFound extends StatelessWidget {
  const _ArtistNotFound({required this.id});

  final String id;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.questionMark(),
              size: 40,
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
            const SizedBox(height: 12),
            const Text('Artist not found'),
            const SizedBox(height: 4),
            SelectableText(
              id,
              style: TextStyle(
                fontSize: 11,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
