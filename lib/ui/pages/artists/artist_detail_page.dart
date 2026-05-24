import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class ArtistDetailPage extends ConsumerWidget {
  const ArtistDetailPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
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
            final albumNames = <String>{for (final s in mine) s.album}.toList()..sort();
            return Column(
              children: [
                BayinPageHeader(
                  title: Text(artist.name),
                  left: BayinGhostIconButton(
                    icon: PhosphorIcons.caretLeft(),
                    tooltip: 'Back',
                    onTap: () {
                      if (context.canPop()) {
                        context.pop();
                      } else {
                        context.go('/artists');
                      }
                    },
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 4, 16, 12),
                  child: _ArtistHeader(
                    artist: artist,
                    songCount: mine.length,
                    albumCount: albumNames.length,
                  ),
                ),
                if (albumNames.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
                    child: BayinGlassCard(
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      child: _AlbumChips(albumNames: albumNames),
                    ),
                  ),
                Expanded(
                  child: Container(
                    margin: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                    decoration: BoxDecoration(
                      color: FlatColors.surfaceContainerHigh(
                        Theme.of(context).brightness,
                      ),
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(
                        color: Theme.of(context).brightness == Brightness.dark
                            ? FlatColors.borderDark
                            : FlatColors.borderLight,
                        width: 0.6,
                      ),
                    ),
                    child: mine.isEmpty
                        ? Center(
                            child: Text(
                              'No songs found for "${artist.name}".',
                              style: TextStyle(
                                color: tokens.textSecondary,
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

class _ArtistHeader extends ConsumerWidget {
  const _ArtistHeader({
    required this.artist,
    required this.songCount,
    required this.albumCount,
  });

  final Artist artist;
  final int songCount;
  final int albumCount;

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
            coverHash: artist.coverHash,
            streamCoverUrl: artist.streamCoverUrl,
            size: CoverArtSize.mid,
            shape: BoxShape.circle,
            placeholderIcon: PhosphorIcons.microphone(),
            placeholderIconSize: 40,
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  artist.name,
                  style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w600),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 4),
                Text(
                  '$albumCount albums',
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

class _AlbumChips extends ConsumerWidget {
  const _AlbumChips({required this.albumNames});

  final List<String> albumNames;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return SizedBox(
      height: 44,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 16),
        itemCount: albumNames.length,
        separatorBuilder: (_, _) => const SizedBox(width: 8),
        itemBuilder: (context, index) {
          final name = albumNames[index];
          return Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
            decoration: BoxDecoration(
              color: tokens.isDark
                  ? Colors.white.withValues(alpha: 0.08)
                  : Colors.black.withValues(alpha: 0.06),
              borderRadius: BorderRadius.circular(20),
              border: Border.all(
                color: tokens.isDark
                    ? Colors.white.withValues(alpha: 0.12)
                    : Colors.black.withValues(alpha: 0.1),
              ),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(PhosphorIcons.vinylRecord(), size: 14),
                const SizedBox(width: 6),
                Text(name, overflow: TextOverflow.ellipsis),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _ArtistNotFound extends ConsumerWidget {
  const _ArtistNotFound({required this.id});

  final String id;

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
            const Text('Artist not found'),
            const SizedBox(height: 4),
            SelectableText(
              id,
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
