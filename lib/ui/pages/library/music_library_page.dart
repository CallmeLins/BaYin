import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as mat show SelectableText;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class MusicLibraryPage extends ConsumerWidget {
  const MusicLibraryPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final songsAsync = ref.watch(librarySongsProvider);
    final albumsAsync = ref.watch(libraryAlbumsProvider);
    final artistsAsync = ref.watch(libraryArtistsProvider);

    if (songsAsync.hasError || albumsAsync.hasError || artistsAsync.hasError) {
      final error = songsAsync.error ?? albumsAsync.error ?? artistsAsync.error ?? 'Unknown error';
      return _ErrorState(error: error);
    }

    final songs = songsAsync.valueOrNull;
    final albums = albumsAsync.valueOrNull;
    final artists = artistsAsync.valueOrNull;
    if (songs == null || albums == null || artists == null) {
      return const Center(child: CircularProgressIndicator());
    }

    final metrics = _buildMetrics(songs);
    return Column(
      children: [
        const BayinPageHeader(
          title: Text('Music Library'),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
              const BayinSectionHeader(title: 'LIBRARY STATS'),
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(12),
                  child: GridView(
                    shrinkWrap: true,
                    physics: const NeverScrollableScrollPhysics(),
                    gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                      maxCrossAxisExtent: 220,
                      mainAxisExtent: 100,
                      mainAxisSpacing: 12,
                      crossAxisSpacing: 12,
                    ),
                    children: [
                      _StatCard(icon: PhosphorIcons.musicNotes(), label: 'Songs', value: '${metrics.totalSongs}'),
                      _StatCard(icon: PhosphorIcons.vinylRecord(), label: 'Albums', value: '${albums.length}'),
                      _StatCard(icon: PhosphorIcons.microphoneStage(), label: 'Artists', value: '${artists.length}'),
                      _StatCard(
                        icon: PhosphorIcons.hardDrives(),
                        label: 'Library Size',
                        value: _formatBytes(metrics.totalBytes),
                      ),
                      _StatCard(icon: PhosphorIcons.folderSimple(), label: 'Local Songs', value: '${metrics.localSongs}'),
                      _StatCard(icon: PhosphorIcons.broadcast(), label: 'Stream Songs', value: '${metrics.streamSongs}'),
                      _StatCard(
                        icon: PhosphorIcons.clockCounterClockwise(),
                        label: 'Total Duration',
                        value: _formatDuration(metrics.totalDurationSeconds),
                      ),
                      _StatCard(
                        icon: PhosphorIcons.imageSquare(),
                        label: 'Songs With Cover',
                        value: '${metrics.withCoverSongs} (${_formatPercent(metrics.coverCoverage)})',
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _StatCard extends ConsumerWidget {
  const _StatCard({
    required this.icon,
    required this.label,
    required this.value,
  });

  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final surfaceColor = tokens.isDark
        ? Colors.white.withValues(alpha: 0.06)
        : Colors.black.withValues(alpha: 0.04);
    final iconBgColor = tokens.isDark
        ? Colors.white.withValues(alpha: 0.08)
        : Colors.black.withValues(alpha: 0.06);
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
      decoration: BoxDecoration(
        color: surfaceColor,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(
          color: tokens.isDark
              ? Colors.white.withValues(alpha: 0.08)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.8,
        ),
      ),
      child: Row(
        children: [
          Container(
            width: 38,
            height: 38,
            decoration: BoxDecoration(
              color: iconBgColor,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(
                color: tokens.isDark
                    ? Colors.white.withValues(alpha: 0.08)
                    : Colors.black.withValues(alpha: 0.06),
                width: 0.8,
              ),
            ),
            child: Icon(icon, size: 18, color: tokens.textPrimary),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  value,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    fontSize: 19,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 11.5,
                    color: tokens.textSecondary,
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

class _ErrorState extends ConsumerWidget {
  const _ErrorState({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.warningCircle(),
              size: 40,
              color: Colors.red,
            ),
            const SizedBox(height: 10),
            Text(
              'Failed to load library stats',
              style: TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w500,
                color: tokens.textPrimary,
              ),
            ),
            const SizedBox(height: 6),
            mat.SelectableText(
              '$error',
              textAlign: TextAlign.center,
              style: TextStyle(color: tokens.textSecondary),
            ),
          ],
        ),
      ),
    );
  }
}

class _LibraryMetrics {
  const _LibraryMetrics({
    required this.totalSongs,
    required this.localSongs,
    required this.streamSongs,
    required this.totalBytes,
    required this.totalDurationSeconds,
    required this.withCoverSongs,
  });

  final int totalSongs;
  final int localSongs;
  final int streamSongs;
  final int totalBytes;
  final int totalDurationSeconds;
  final int withCoverSongs;

  double get coverCoverage => totalSongs == 0 ? 0 : withCoverSongs / totalSongs;
}

_LibraryMetrics _buildMetrics(List<Song> songs) {
  var localSongs = 0;
  var streamSongs = 0;
  var totalBytes = 0;
  var totalDurationSeconds = 0;
  var withCoverSongs = 0;

  for (final song in songs) {
    if (song.sourceType == 'stream') {
      streamSongs += 1;
    } else {
      localSongs += 1;
    }
    totalBytes += song.fileSize;
    totalDurationSeconds += song.duration.round();
    final hash = song.coverHash;
    if (hash != null && hash.isNotEmpty) {
      withCoverSongs += 1;
    }
  }

  return _LibraryMetrics(
    totalSongs: songs.length,
    localSongs: localSongs,
    streamSongs: streamSongs,
    totalBytes: totalBytes,
    totalDurationSeconds: totalDurationSeconds,
    withCoverSongs: withCoverSongs,
  );
}

String _formatDuration(int totalSeconds) {
  final hours = totalSeconds ~/ 3600;
  final minutes = (totalSeconds % 3600) ~/ 60;
  if (hours > 0) {
    return '${hours}h ${minutes}m';
  }
  return '${minutes}m';
}

String _formatBytes(int bytes) {
  if (bytes <= 0) return '0 B';
  const units = <String>['B', 'KB', 'MB', 'GB', 'TB'];
  var value = bytes.toDouble();
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  final fractionDigits = value >= 100 ? 0 : 1;
  return '${value.toStringAsFixed(fractionDigits)} ${units[unit]}';
}

String _formatPercent(double ratio) {
  return '${(ratio * 100).toStringAsFixed(0)}%';
}
