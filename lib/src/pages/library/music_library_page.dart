import 'package:flutter/material.dart';
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
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 4),
              GridView(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                  maxCrossAxisExtent: 220,
                  mainAxisExtent: 100,
                  mainAxisSpacing: 10,
                  crossAxisSpacing: 10,
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
            ],
          ),
        ),
      ],
    );
  }
}

class _StatCard extends StatelessWidget {
  const _StatCard({
    required this.icon,
    required this.label,
    required this.value,
  });

  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          Container(
            width: 36,
            height: 36,
            decoration: BoxDecoration(
              color: scheme.secondaryContainer,
              borderRadius: BorderRadius.circular(10),
            ),
            child: Icon(icon, size: 18, color: scheme.onSecondaryContainer),
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
                    fontSize: 18,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 12,
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

class _ErrorState extends StatelessWidget {
  const _ErrorState({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.warningCircle(),
              size: 40,
              color: Theme.of(context).colorScheme.error,
            ),
            const SizedBox(height: 10),
            Text(
              'Failed to load library stats',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 6),
            SelectableText(
              '$error',
              textAlign: TextAlign.center,
              style: TextStyle(color: Theme.of(context).colorScheme.onSurfaceVariant),
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
