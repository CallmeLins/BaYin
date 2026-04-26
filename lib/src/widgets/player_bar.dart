import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../models/models.dart';
import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';
import 'cover_art.dart';

const double kPlayerBarWideHeight = 88;
const double kPlayerBarCompactHeight = 96;

class PlayerBar extends ConsumerWidget {
  const PlayerBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final layout = ref.watch(responsiveLayoutProvider);
    final player = ref.watch(playerControllerProvider);
    final controller = ref.read(playerControllerProvider.notifier);
    final tokens = Theme.of(context).extension<BayinTokens>()!;

    final isCompact = layout.isCompact;
    final baseHeight = isCompact ? kPlayerBarCompactHeight : kPlayerBarWideHeight;
    final safeBottom = isCompact ? MediaQuery.paddingOf(context).bottom : 0.0;

    return Material(
      color: tokens.barBg,
      child: Container(
        height: baseHeight + safeBottom,
        padding: EdgeInsets.fromLTRB(12, 8, 12, 8 + safeBottom),
        decoration: BoxDecoration(
          border: Border(
            top: BorderSide(color: tokens.separatorColor, width: 0.5),
          ),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            _SongSlot(
              song: player.currentSong,
              isPlaying: player.isPlaying,
              compact: isCompact,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _TransportCluster(
                state: player,
                compact: isCompact,
                onPrevious: controller.previous,
                onToggle: controller.togglePlayPause,
                onNext: controller.next,
              ),
            ),
            const SizedBox(width: 12),
            _SecondaryActions(
              state: player,
              compact: isCompact,
              onVolumeChanged: controller.setVolume,
            ),
          ],
        ),
      ),
    );
  }
}

class _SongSlot extends StatelessWidget {
  const _SongSlot({
    required this.song,
    required this.isPlaying,
    required this.compact,
  });

  final Song? song;
  final bool isPlaying;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final title = song?.title ?? 'Not playing';
    final subtitle = song == null
        ? 'Select a song'
        : '${song!.artist} - ${song!.album}';

    return SizedBox(
      width: compact ? 180 : 260,
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        onTap: song == null ? null : () => context.go('/player'),
        child: Row(
          children: [
            Container(
              width: 48,
              height: 48,
              clipBehavior: Clip.antiAlias,
              decoration: BoxDecoration(borderRadius: BorderRadius.circular(8)),
              child: CoverArt(
                width: 48,
                height: 48,
                coverHash: song?.coverHash,
                streamInfo: song?.streamInfo,
                size: CoverArtSize.small,
                borderRadius: BorderRadius.circular(8),
                placeholderIcon: isPlaying
                    ? PhosphorIcons.musicNotes(PhosphorIconsStyle.fill)
                    : PhosphorIcons.musicNote(),
                placeholderIconSize: 20,
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
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
          ],
        ),
      ),
    );
  }
}

class _TransportCluster extends StatelessWidget {
  const _TransportCluster({
    required this.state,
    required this.compact,
    required this.onPrevious,
    required this.onToggle,
    required this.onNext,
  });

  final PlayerControllerState state;
  final bool compact;
  final Future<void> Function() onPrevious;
  final Future<void> Function() onToggle;
  final Future<void> Function() onNext;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final canControl = state.currentSong != null || state.queue.isNotEmpty;

    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            IconButton(
              tooltip: 'Previous',
              icon: Icon(PhosphorIcons.skipBack()),
              iconSize: 20,
              color: scheme.onSurface,
              onPressed: canControl ? onPrevious : null,
            ),
            IconButton.filled(
              tooltip: state.isPlaying ? 'Pause' : 'Play',
              icon: Icon(
                state.isPlaying
                    ? PhosphorIcons.pause(PhosphorIconsStyle.fill)
                    : PhosphorIcons.play(PhosphorIconsStyle.fill),
              ),
              iconSize: 18,
              onPressed: canControl ? onToggle : null,
            ),
            IconButton(
              tooltip: 'Next',
              icon: Icon(PhosphorIcons.skipForward()),
              iconSize: 20,
              color: scheme.onSurface,
              onPressed: canControl ? onNext : null,
            ),
          ],
        ),
        const SizedBox(height: 4),
        _ProgressSlider(
          positionSecs: state.positionSecs,
          durationSecs: state.durationSecs,
          compact: compact,
          enabled: state.currentSong != null,
        ),
      ],
    );
  }
}

class _SecondaryActions extends StatelessWidget {
  const _SecondaryActions({
    required this.state,
    required this.compact,
    required this.onVolumeChanged,
  });

  final PlayerControllerState state;
  final bool compact;
  final Future<void> Function(double) onVolumeChanged;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (!compact) ...[
          Icon(
            state.volume <= 0.01
                ? PhosphorIcons.speakerSlash()
                : PhosphorIcons.speakerHigh(),
            size: 18,
            color: scheme.onSurfaceVariant,
          ),
          SizedBox(
            width: 110,
            child: Slider(
              value: state.volume.clamp(0.0, 1.0),
              min: 0,
              max: 1,
              onChanged: onVolumeChanged,
            ),
          ),
        ],
        IconButton(
          tooltip: 'Open player',
          icon: Icon(PhosphorIcons.queue()),
          color: scheme.onSurfaceVariant,
          onPressed: () => context.go('/player'),
        ),
      ],
    );
  }
}

class _ProgressSlider extends ConsumerStatefulWidget {
  const _ProgressSlider({
    required this.positionSecs,
    required this.durationSecs,
    required this.compact,
    required this.enabled,
  });

  final double positionSecs;
  final double durationSecs;
  final bool compact;
  final bool enabled;

  @override
  ConsumerState<_ProgressSlider> createState() => _ProgressSliderState();
}

class _ProgressSliderState extends ConsumerState<_ProgressSlider> {
  double? _dragValue;

  @override
  Widget build(BuildContext context) {
    final max = widget.durationSecs > 0 ? widget.durationSecs : 1.0;
    final value = (_dragValue ?? widget.positionSecs).clamp(0.0, max);
    final scheme = Theme.of(context).colorScheme;

    return Row(
      children: [
        if (!widget.compact)
          SizedBox(
            width: 46,
            child: Text(
              _formatDuration(widget.positionSecs),
              style: TextStyle(
                fontSize: 11,
                color: scheme.onSurfaceVariant,
                fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
              ),
            ),
          ),
        Expanded(
          child: SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 3,
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 5),
              overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
            ),
            child: Slider(
              value: value,
              min: 0,
              max: max,
              onChanged: widget.enabled
                  ? (next) => setState(() => _dragValue = next)
                  : null,
              onChangeEnd: widget.enabled
                  ? (next) {
                      setState(() => _dragValue = null);
                      ref.read(playerControllerProvider.notifier).seek(next);
                    }
                  : null,
            ),
          ),
        ),
        if (!widget.compact)
          SizedBox(
            width: 46,
            child: Text(
              _formatDuration(widget.durationSecs),
              textAlign: TextAlign.right,
              style: TextStyle(
                fontSize: 11,
                color: scheme.onSurfaceVariant,
                fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
              ),
            ),
          ),
      ],
    );
  }
}

String _formatDuration(double seconds) {
  if (seconds.isNaN || seconds < 0) {
    return '--:--';
  }
  final total = seconds.round();
  final hours = total ~/ 3600;
  final minutes = (total % 3600) ~/ 60;
  final secs = total % 60;
  if (hours > 0) {
    return '$hours:${minutes.toString().padLeft(2, '0')}:${secs.toString().padLeft(2, '0')}';
  }
  return '${minutes.toString().padLeft(2, '0')}:${secs.toString().padLeft(2, '0')}';
}
