import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

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
                BayinPageHeader(
                  title: Text(album.name),
                  left: Tooltip(
                    message: 'Back',
                    child: IconButton(
                      onPressed: () {
                        if (context.canPop()) {
                          context.pop();
                        } else {
                          context.go('/albums');
                        }
                      },
                      icon: Icon(PhosphorIcons.caretLeft()),
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 4, 12, 10),
                  child: _AlbumHeader(album: album, songCount: inAlbum.length),
                ),
                Expanded(
                  child: BayinGlassCard(
                    margin: const EdgeInsets.fromLTRB(8, 0, 8, 10),
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
                            onLongPress: (song) => SongMenu.show(context, song: song),
                          ),
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

class _AlbumHeader extends ConsumerWidget {
  const _AlbumHeader({required this.album, required this.songCount});

  final Album album;
  final int songCount;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return BayinGlassCard(
      padding: const EdgeInsets.all(14),
      child: Row(
        children: [
          CoverArt(
            width: 92,
            height: 92,
            coverHash: album.coverHash,
            streamCoverUrl: album.streamCoverUrl,
            size: CoverArtSize.mid,
            borderRadius: BorderRadius.circular(12),
            placeholderIcon: PhosphorIcons.vinylRecord(),
            placeholderIconSize: 40,
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  album.name,
                  style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w600),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 4),
                Text(
                  album.artist,
                  style: TextStyle(
                    color: const Color(0xFF3B82F6),
                    fontWeight: FontWeight.w500,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  '$songCount tracks',
                  style: TextStyle(color: tokens.textSecondary),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _AlbumNotFound extends ConsumerWidget {
  const _AlbumNotFound({required this.albumId});

  final String albumId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.questionMark(),
              size: 40,
              color: tokens.textSecondary,
            ),
            const SizedBox(height: 12),
            const Text('Album not found'),
            const SizedBox(height: 4),
            SelectableText(
              albumId,
              style: TextStyle(
                fontSize: 11,
                color: tokens.textSecondary,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptySongsInAlbum extends ConsumerWidget {
  const _EmptySongsInAlbum({required this.album});

  final Album album;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          'No songs found for "${album.name}".',
          style: TextStyle(
            color: tokens.textSecondary,
          ),
        ),
      ),
    );
  }
}
