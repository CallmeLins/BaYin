import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — one album's songs.
class AlbumDetailPage extends ConsumerWidget {
  const AlbumDetailPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final albumId = Uri.decodeComponent(
      GoRouterState.of(context).pathParameters['albumId'] ?? '',
    );
    final asyncAlbums = ref.watch(libraryAlbumsProvider);
    final asyncSongs = ref.watch(librarySongsProvider);

    return asyncAlbums.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load album\n$error'),
      ),
      data: (albums) {
        final Album? album =
            albums.where((a) => a.id == albumId).cast<Album?>().firstOrNull;
        if (album == null) {
          return _AlbumNotFound(albumId: albumId);
        }
        return asyncSongs.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(
            child: SelectableText('Failed to load songs\n$error'),
          ),
          data: (songs) {
            final inAlbum =
                songs.where((s) => s.album == album.name).toList(growable: false);
            return Column(
              children: [
                _AlbumHeader(album: album, songCount: inAlbum.length),
                Expanded(
                  child: inAlbum.isEmpty
                      ? _EmptySongsInAlbum(album: album)
                      : SongList(
                          songs: inAlbum,
                          showIndex: true,
                          onTap: (song) => ref
                              .read(playerControllerProvider.notifier)
                              .playQueue(
                                inAlbum,
                                startIndex: inAlbum.indexOf(song),
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

class _AlbumHeader extends StatelessWidget {
  const _AlbumHeader({required this.album, required this.songCount});

  final Album album;
  final int songCount;

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
                context.go('/albums');
              }
            },
            icon: Icon(PhosphorIcons.arrowLeft()),
            tooltip: 'Back',
          ),
          const SizedBox(width: 4),
          CoverArt(
            width: 64,
            height: 64,
            coverHash: album.coverHash,
            streamCoverUrl: album.streamCoverUrl,
            size: CoverArtSize.mid,
            borderRadius: BorderRadius.circular(8),
            placeholderIcon: PhosphorIcons.vinylRecord(),
            placeholderIconSize: 30,
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  album.name,
                  style: Theme.of(context).textTheme.titleLarge,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 4),
                Text(
                  '${album.artist} · $songCount tracks',
                  style: TextStyle(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _AlbumNotFound extends StatelessWidget {
  const _AlbumNotFound({required this.albumId});

  final String albumId;

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
            const Text('Album not found'),
            const SizedBox(height: 4),
            SelectableText(
              albumId,
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

class _EmptySongsInAlbum extends StatelessWidget {
  const _EmptySongsInAlbum({required this.album});

  final Album album;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          'No songs found for "${album.name}".',
          style: TextStyle(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
      ),
    );
  }
}
