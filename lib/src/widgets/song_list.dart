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

/// Song list widget — fills the entire content area.
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
    return ClipRect(
      child: ListView.builder(
        controller: scrollController,
        padding: padding ?? EdgeInsets.zero,
        itemCount: songs.length,
        itemBuilder: (context, index) {
          final song = songs[index];
          return _SongRow(
            song: song,
            index: showIndex ? index + 1 : null,
            onTap: onTap == null ? null : () => onTap!(song),
            onLongPress: onLongPress == null ? null : () => onLongPress!(song),
          );
        },
      ),
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
    final isOdd = (index ?? 0).isOdd;

    return GestureDetector(
      onTap: onTap,
      onLongPress: onLongPress,
      child: Container(
        height: 44,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        // Zebra striping.
        color: isOdd
            ? FlatColors.stateLayer(brightness, FlatStateIntensity.subtle)
            : Colors.transparent,
        child: Row(
          children: [
            // Index number.
            if (index != null)
              SizedBox(
                width: 32,
                child: Text(
                  '$index',
                  textAlign: TextAlign.center,
                  style: FlatTypography.caption(brightness).copyWith(
                    fontFeatures: const <FontFeature>[
                      FontFeature.tabularFigures(),
                    ],
                  ),
                ),
              ),

            // Cover art.
            ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: CoverArt(
                width: 32,
                height: 32,
                coverHash: song.coverHash,
                streamInfo: song.streamInfo,
                size: CoverArtSize.small,
                borderRadius: BorderRadius.circular(4),
                placeholderIconSize: 14,
              ),
            ),
            const SizedBox(width: 10),

            // Song info.
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    song.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: FlatTypography.bodySmall(brightness).copyWith(
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  Text(
                    '${song.artist} \u00b7 ${song.album}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: FlatTypography.caption(brightness),
                  ),
                ],
              ),
            ),

            const SizedBox(width: 12),

            // Duration.
            Text(
              _formatDuration(song.duration),
              style: FlatTypography.caption(brightness).copyWith(
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
