import 'package:flutter/material.dart';

import '../models/models.dart';
import 'cover_art.dart';

String _formatDuration(double seconds) {
  if (seconds.isNaN || seconds < 0) return '--:--';
  final total = seconds.round();
  final h = total ~/ 3600;
  final m = (total % 3600) ~/ 60;
  final s = total % 60;
  if (h > 0) {
    return '$h:${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
  }
  return '${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
}

/// Virtualized Song row list. Used by SongsPage, AlbumDetailPage,
/// PlaylistDetailPage, and search results.
class SongList extends StatelessWidget {
  const SongList({
    super.key,
    required this.songs,
    this.onTap,
    this.onLongPress,
    this.scrollController,
    this.padding,
    this.showIndex = false,
  });

  final List<Song> songs;
  final ValueChanged<Song>? onTap;
  final ValueChanged<Song>? onLongPress;
  final ScrollController? scrollController;
  final EdgeInsets? padding;
  final bool showIndex;

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      controller: scrollController,
      padding: padding ?? const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      itemCount: songs.length,
      itemExtent: 52,
      itemBuilder: (context, index) {
        final song = songs[index];
        return _SongRow(
          song: song,
          index: showIndex ? index + 1 : null,
          onTap: onTap == null ? null : () => onTap!(song),
          onLongPress: onLongPress == null ? null : () => onLongPress!(song),
        );
      },
    );
  }
}

class _SongRow extends StatelessWidget {
  const _SongRow({
    required this.song,
    required this.index,
    required this.onTap,
    required this.onLongPress,
  });

  final Song song;
  final int? index;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(6),
        onTap: onTap,
        onLongPress: onLongPress,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          child: Row(
            children: [
              if (index != null)
                SizedBox(
                  width: 36,
                  child: Text(
                    '$index',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 12,
                      color: scheme.onSurfaceVariant,
                      fontFeatures: const <FontFeature>[
                        FontFeature.tabularFigures(),
                      ],
                    ),
                  ),
                ),
              CoverArt(
                width: 36,
                height: 36,
                coverHash: song.coverHash,
                streamInfo: song.streamInfo,
                size: CoverArtSize.small,
                borderRadius: BorderRadius.circular(4),
                placeholderIconSize: 16,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      song.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      '${song.artist} · ${song.album}',
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
              const SizedBox(width: 8),
              Text(
                _formatDuration(song.duration),
                style: TextStyle(
                  fontSize: 12,
                  color: scheme.onSurfaceVariant,
                  fontFeatures: const <FontFeature>[
                    FontFeature.tabularFigures(),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
