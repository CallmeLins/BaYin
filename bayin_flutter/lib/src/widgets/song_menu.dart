import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../models/models.dart';
import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';

/// Long-press / right-click action sheet for a [Song].
///
/// Phase 3 subset: Song Info / Go to Album / Go to Artist are wired.
/// Queue, playlist, and delete actions wait for Phase 4 (player + CRUD) and
/// show a Coming-soon snackbar for now.
class SongMenu {
  const SongMenu._();

  static Future<void> show(BuildContext context, {required Song song}) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      barrierColor: Colors.black.withValues(alpha: 0.35),
      builder: (sheetContext) => _SongMenuSheet(song: song),
    );
  }
}

enum _MenuView { main, info }

class _SongMenuSheet extends ConsumerStatefulWidget {
  const _SongMenuSheet({required this.song});

  final Song song;

  @override
  ConsumerState<_SongMenuSheet> createState() => _SongMenuSheetState();
}

class _SongMenuSheetState extends ConsumerState<_SongMenuSheet> {
  _MenuView _view = _MenuView.main;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final tokens = Theme.of(context).extension<BayinTokens>();
    final mq = MediaQuery.of(context);

    return SafeArea(
      top: false,
      child: Padding(
        padding: EdgeInsets.only(bottom: mq.viewInsets.bottom),
        child: Container(
          constraints: BoxConstraints(maxHeight: mq.size.height * 0.82),
          margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
          decoration: BoxDecoration(
            color: tokens?.popoverBg ?? scheme.surface,
            borderRadius: BorderRadius.circular(18),
            border: Border.all(color: scheme.outlineVariant, width: 0.5),
            boxShadow: const [
              BoxShadow(
                color: Color(0x40000000),
                blurRadius: 24,
                offset: Offset(0, 8),
              ),
            ],
          ),
          clipBehavior: Clip.antiAlias,
          child: _buildContent(context),
        ),
      ),
    );
  }

  Widget _buildContent(BuildContext context) {
    switch (_view) {
      case _MenuView.info:
        return _InfoView(
          song: widget.song,
          onBack: () => setState(() => _view = _MenuView.main),
        );
      case _MenuView.main:
        return _MainView(
          song: widget.song,
          onShowInfo: () => setState(() => _view = _MenuView.info),
        );
    }
  }
}

class _MainView extends ConsumerWidget {
  const _MainView({required this.song, required this.onShowInfo});

  final Song song;
  final VoidCallback onShowInfo;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        _SongHeader(
          song: song,
          onClose: () => Navigator.of(context).maybePop(),
        ),
        Divider(height: 1, color: scheme.outlineVariant.withValues(alpha: 0.4)),
        Flexible(
          child: ListView(
            padding: const EdgeInsets.symmetric(vertical: 8),
            shrinkWrap: true,
            children: [
              _MenuRow(
                icon: PhosphorIcons.playCircle(),
                label: 'Play next',
                onTap: () => _enqueueNext(context, ref),
              ),
              _MenuRow(
                icon: PhosphorIcons.plusCircle(),
                label: 'Add to playlist',
                onTap: () => _notYet(context, 'Playlist CRUD lands in Phase 4'),
              ),
              _MenuRow(
                icon: PhosphorIcons.user(),
                label: 'Go to artist',
                onTap: () => _goToArtist(context, ref),
              ),
              _MenuRow(
                icon: PhosphorIcons.vinylRecord(),
                label: 'Go to album',
                onTap: () => _goToAlbum(context, ref),
              ),
              _MenuRow(
                icon: PhosphorIcons.info(),
                label: 'Song info',
                onTap: onShowInfo,
              ),
              _MenuRow(
                icon: PhosphorIcons.trash(),
                label: 'Delete from library',
                danger: true,
                onTap: () => _notYet(
                  context,
                  'Per-song delete lands in Phase 4',
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  Future<void> _goToArtist(BuildContext context, WidgetRef ref) async {
    final artists = await ref.read(libraryArtistsProvider.future);
    if (!context.mounted) return;
    final match = artists.where((a) => a.name == song.artist).cast<Artist?>().firstOrNull;
    if (match == null) {
      _notYet(context, 'No artist record for "${song.artist}"');
      return;
    }
    Navigator.of(context).maybePop();
    if (!context.mounted) return;
    context.go('/artists/${Uri.encodeComponent(match.id)}');
  }

  Future<void> _goToAlbum(BuildContext context, WidgetRef ref) async {
    final albums = await ref.read(libraryAlbumsProvider.future);
    if (!context.mounted) return;
    final match = albums.where((a) => a.name == song.album).cast<Album?>().firstOrNull;
    if (match == null) {
      _notYet(context, 'No album record for "${song.album}"');
      return;
    }
    Navigator.of(context).maybePop();
    if (!context.mounted) return;
    context.go('/albums/${Uri.encodeComponent(match.id)}');
  }

  void _notYet(BuildContext context, String message) {
    Navigator.of(context).maybePop();
    final messenger = ScaffoldMessenger.maybeOf(context);
    messenger?.showSnackBar(
      SnackBar(
        content: Text(message),
        behavior: SnackBarBehavior.floating,
        duration: const Duration(seconds: 2),
      ),
    );
  }

  void _enqueueNext(BuildContext context, WidgetRef ref) {
    ref.read(playerControllerProvider.notifier).enqueueNext(song);
    Navigator.of(context).maybePop();
    final messenger = ScaffoldMessenger.maybeOf(context);
    messenger?.showSnackBar(
      const SnackBar(
        content: Text('Added to play next'),
        behavior: SnackBarBehavior.floating,
        duration: Duration(seconds: 2),
      ),
    );
  }
}

class _InfoView extends StatelessWidget {
  const _InfoView({required this.song, required this.onBack});

  final Song song;
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        _SubHeader(title: 'Song info', onBack: onBack),
        Divider(height: 1, color: scheme.outlineVariant.withValues(alpha: 0.4)),
        Flexible(
          child: ListView(
            padding: const EdgeInsets.all(16),
            shrinkWrap: true,
            children: [
              _SongHero(song: song),
              const SizedBox(height: 18),
              _InfoRow(label: 'Title', value: song.title),
              _InfoRow(label: 'Artist', value: song.artist),
              _InfoRow(label: 'Album', value: song.album),
              _InfoRow(label: 'Duration', value: _formatDuration(song.duration)),
              _InfoRow(label: 'File size', value: _formatSize(song.fileSize)),
              if (song.format != null)
                _InfoRow(label: 'Format', value: song.format!.toUpperCase()),
              if (song.bitrate != null)
                _InfoRow(label: 'Bitrate', value: '${song.bitrate} kbps'),
              if (song.sampleRate != null)
                _InfoRow(
                  label: 'Sample rate',
                  value: '${(song.sampleRate! / 1000).toStringAsFixed(1)} kHz',
                ),
              if (song.bitDepth != null)
                _InfoRow(label: 'Bit depth', value: '${song.bitDepth}-bit'),
              if (song.channels != null)
                _InfoRow(label: 'Channels', value: '${song.channels}'),
              if (song.isHr == true)
                const _InfoRow(label: 'Quality', value: 'Hi-Res'),
              if (song.isSq == true)
                const _InfoRow(label: 'Quality', value: 'Super quality'),
              const SizedBox(height: 12),
              _PathBlock(filePath: song.filePath, sourceType: song.sourceType),
            ],
          ),
        ),
      ],
    );
  }
}

class _SongHeader extends StatelessWidget {
  const _SongHeader({required this.song, required this.onClose});

  final Song song;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 8, 12),
      child: Row(
        children: [
          Container(
            width: 48,
            height: 48,
            decoration: BoxDecoration(
              color: scheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(8),
            ),
            alignment: Alignment.center,
            child: Icon(
              PhosphorIcons.musicNote(),
              size: 22,
              color: scheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  song.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  '${song.artist} · ${song.album}',
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
          IconButton(
            onPressed: onClose,
            icon: Icon(PhosphorIcons.x()),
            tooltip: 'Close',
          ),
        ],
      ),
    );
  }
}

class _SubHeader extends StatelessWidget {
  const _SubHeader({required this.title, required this.onBack});

  final String title;
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(8, 8, 16, 8),
      child: Row(
        children: [
          IconButton(
            onPressed: onBack,
            icon: Icon(PhosphorIcons.caretLeft()),
            tooltip: 'Back',
          ),
          const SizedBox(width: 4),
          Text(
            title,
            style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );
  }
}

class _MenuRow extends StatelessWidget {
  const _MenuRow({
    required this.icon,
    required this.label,
    required this.onTap,
    this.danger = false,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final color = danger ? scheme.error : scheme.onSurface;
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        child: Row(
          children: [
            Icon(icon, size: 18, color: color.withValues(alpha: 0.85)),
            const SizedBox(width: 14),
            Expanded(
              child: Text(
                label,
                style: TextStyle(
                  fontSize: 13.5,
                  color: color,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SongHero extends StatelessWidget {
  const _SongHero({required this.song});

  final Song song;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Row(
      children: [
        Container(
          width: 64,
          height: 64,
          decoration: BoxDecoration(
            color: scheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(10),
          ),
          alignment: Alignment.center,
          child: Icon(
            PhosphorIcons.musicNote(),
            size: 28,
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
                song.title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w600,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                song.artist,
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
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 96,
            child: Text(
              label,
              style: TextStyle(
                fontSize: 12,
                color: scheme.onSurfaceVariant,
              ),
            ),
          ),
          Expanded(
            child: SelectableText(
              value,
              style: const TextStyle(fontSize: 12.5),
            ),
          ),
        ],
      ),
    );
  }
}

class _PathBlock extends StatelessWidget {
  const _PathBlock({required this.filePath, required this.sourceType});

  final String filePath;
  final String sourceType;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final parsed = _sanitizePath(filePath);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHighest.withValues(alpha: 0.6),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            parsed.isRemote ? 'Source' : 'File path',
            style: TextStyle(
              fontSize: 11,
              color: scheme.onSurfaceVariant,
              fontWeight: FontWeight.w500,
            ),
          ),
          const SizedBox(height: 4),
          SelectableText(
            parsed.display,
            style: const TextStyle(fontSize: 12),
          ),
        ],
      ),
    );
  }
}

class _SanitizedPath {
  const _SanitizedPath(this.display, {required this.isRemote});
  final String display;
  final bool isRemote;
}

_SanitizedPath _sanitizePath(String filePath) {
  // Stream songs store a JSON blob in filePath (see src-ui SongMenu.tsx).
  final trimmed = filePath.trim();
  if (trimmed.startsWith('{')) {
    return const _SanitizedPath('Remote stream source', isRemote: true);
  }
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    final uri = Uri.tryParse(trimmed);
    if (uri != null && uri.host.isNotEmpty) {
      return _SanitizedPath('${uri.host} (Remote)', isRemote: true);
    }
    return const _SanitizedPath('Remote source', isRemote: true);
  }
  return _SanitizedPath(filePath, isRemote: false);
}

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

String _formatSize(int bytes) {
  if (bytes <= 0) return 'Unknown';
  const units = ['B', 'KB', 'MB', 'GB'];
  var value = bytes.toDouble();
  var i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return '${value.toStringAsFixed(value >= 10 || i == 0 ? 0 : 1)} ${units[i]}';
}
