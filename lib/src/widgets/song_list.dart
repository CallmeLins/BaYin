import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../theme/design_tokens.dart';
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

class SongList extends ConsumerWidget {
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
  Widget build(BuildContext context, WidgetRef ref) {
    return ListView.builder(
      controller: scrollController,
      padding: padding ??
          const EdgeInsets.symmetric(
            horizontal: FlatSpacing.sm + 4,
            vertical: FlatSpacing.sm,
          ),
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

class _SongRow extends ConsumerWidget {
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
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return GestureDetector(
      onTap: onTap,
      onLongPress: onLongPress,
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: FlatSpacing.sm,
          vertical: FlatSpacing.xs,
        ),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(FlatRadius.md - 2),
        ),
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
                    color: FlatColors.textSecondary(brightness),
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
              borderRadius: BorderRadius.circular(FlatRadius.sm),
              placeholderIconSize: 16,
            ),
            const SizedBox(width: FlatSpacing.sm + 2),
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
                    '${song.artist} \u00b7 ${song.album}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: FlatTypography.caption(brightness),
                  ),
                ],
              ),
            ),
            const SizedBox(width: FlatSpacing.sm),
            Text(
              _formatDuration(song.duration),
              style: TextStyle(
                fontSize: 12,
                color: FlatColors.textSecondary(brightness),
                fontFeatures: const <FontFeature>[
                  FontFeature.tabularFigures(),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
