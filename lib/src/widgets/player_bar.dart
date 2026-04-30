import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../models/models.dart';
import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';
import 'cover_art.dart';

const double kPlayerBarWideHeight = 72;
const double kPlayerBarCompactHeight = 84;

class PlayerBar extends ConsumerStatefulWidget {
  const PlayerBar({super.key});

  @override
  ConsumerState<PlayerBar> createState() => _PlayerBarState();
}

class _PlayerBarState extends ConsumerState<PlayerBar> {
  final GlobalKey _progressKey = GlobalKey();
  final GlobalKey _volumeKey = GlobalKey();
  double _lastNonZeroVolume = 1;

  @override
  Widget build(BuildContext context) {
    final layout = ref.watch(responsiveLayoutProvider);
    final player = ref.watch(playerControllerProvider);
    final controller = ref.read(playerControllerProvider.notifier);
    final tokens = Theme.of(context).extension<BayinTokens>()!;

    final isCompact = layout.isCompact;
    final safeBottom = isCompact ? MediaQuery.paddingOf(context).bottom : 0.0;
    final height = (isCompact ? kPlayerBarCompactHeight : kPlayerBarWideHeight) +
        safeBottom;

    final song = player.currentSong;
    final canControl = song != null || player.queue.isNotEmpty;
    final effectiveDuration = song == null
        ? 0.0
        : (player.durationSecs > 0 ? player.durationSecs : song.duration);
    final clampedProgress = effectiveDuration > 0
        ? player.positionSecs.clamp(0.0, effectiveDuration)
        : 0.0;
    final progressPercent = effectiveDuration > 0
        ? (clampedProgress / effectiveDuration).clamp(0.0, 1.0)
        : 0.0;

    return Material(
      color: tokens.barBg.withValues(alpha: 0.86),
      child: SizedBox(
        height: height,
        child: Stack(
          children: [
            Positioned(
              top: 0,
              left: 0,
              right: 0,
              child: _ProgressScrubBar(
                key: _progressKey,
                progressPercent: progressPercent,
                onSeek: (fraction) {
                  if (effectiveDuration <= 0) return;
                  final next = fraction * effectiveDuration;
                  unawaited(controller.seek(next));
                },
              ),
            ),
            Positioned.fill(
              child: Padding(
                padding: EdgeInsets.fromLTRB(14, 10, 14, 10 + safeBottom),
                child: Row(
                  children: [
                    Expanded(
                      child: _SongSlot(
                        song: song,
                        onTap: song == null ? null : () => context.go('/player'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    _TransportCluster(
                      isCompact: isCompact,
                      canControl: canControl,
                      isPlaying: player.isPlaying,
                      onPrevious: () => unawaited(controller.previous()),
                      onToggle: () => unawaited(controller.togglePlayPause()),
                      onNext: () => unawaited(controller.next()),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: _RightActions(
                        isCompact: isCompact,
                        queueCount: player.queue.length,
                        volume: player.volume.clamp(0.0, 1.0),
                        volumeKey: _volumeKey,
                        onQueue: () => _openQueueSheet(context, player, controller),
                        onSetVolume: (value) {
                          if (value > 0.001) {
                            _lastNonZeroVolume = value;
                          }
                          unawaited(controller.setVolume(value));
                        },
                        onToggleMute: () {
                          final current = player.volume;
                          if (current <= 0.001) {
                            unawaited(controller.setVolume(_lastNonZeroVolume));
                          } else {
                            _lastNonZeroVolume = current;
                            unawaited(controller.setVolume(0));
                          }
                        },
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _openQueueSheet(
    BuildContext context,
    PlayerControllerState player,
    PlayerController controller,
  ) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      barrierColor: Colors.black.withValues(alpha: 0.42),
      builder: (_) {
        final queue = player.queue;
        return SafeArea(
          top: false,
          child: Container(
            height: MediaQuery.sizeOf(context).height * 0.7,
            decoration: BoxDecoration(
              color: const Color(0xFF1A1A1D).withValues(alpha: 0.94),
              borderRadius: const BorderRadius.vertical(top: Radius.circular(28)),
              border: Border.all(color: Colors.white.withValues(alpha: 0.08)),
            ),
            child: Column(
              children: [
                const SizedBox(height: 10),
                Container(
                  width: 44,
                  height: 4,
                  decoration: BoxDecoration(
                    color: Colors.white.withValues(alpha: 0.24),
                    borderRadius: BorderRadius.circular(999),
                  ),
                ),
                const SizedBox(height: 12),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Row(
                    children: [
                      const Text(
                        'Playing Next',
                        style: TextStyle(
                          color: Colors.white,
                          fontSize: 18,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      const Spacer(),
                      if (queue.isNotEmpty)
                        TextButton(
                          onPressed: () => unawaited(controller.clearQueue()),
                          child: const Text('Clear'),
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 6),
                Expanded(
                  child: queue.isEmpty
                      ? Center(
                          child: Text(
                            'Queue is empty',
                            style: TextStyle(
                              color: Colors.white.withValues(alpha: 0.62),
                            ),
                          ),
                        )
                      : ListView.builder(
                          itemCount: queue.length,
                          padding: const EdgeInsets.fromLTRB(10, 0, 10, 12),
                          itemBuilder: (context, index) {
                            final item = queue[index];
                            final active = player.currentIndex == index;
                            return Padding(
                              padding: const EdgeInsets.symmetric(vertical: 2),
                              child: InkWell(
                                borderRadius: BorderRadius.circular(12),
                                onTap: () {
                                  Navigator.of(context).pop();
                                  unawaited(controller.jumpTo(index));
                                },
                                child: Container(
                                  padding: const EdgeInsets.all(10),
                                  decoration: BoxDecoration(
                                    borderRadius: BorderRadius.circular(12),
                                    color: active
                                        ? Colors.white.withValues(alpha: 0.10)
                                        : Colors.transparent,
                                  ),
                                  child: Row(
                                    children: [
                                      ClipRRect(
                                        borderRadius: BorderRadius.circular(8),
                                        child: CoverArt(
                                          width: 42,
                                          height: 42,
                                          coverHash: item.coverHash,
                                          streamInfo: item.streamInfo,
                                          size: CoverArtSize.small,
                                        ),
                                      ),
                                      const SizedBox(width: 10),
                                      Expanded(
                                        child: Column(
                                          crossAxisAlignment: CrossAxisAlignment.start,
                                          children: [
                                            Text(
                                              item.title,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: TextStyle(
                                                color: active
                                                    ? const Color(0xFF60A5FA)
                                                    : Colors.white,
                                                fontWeight: FontWeight.w600,
                                              ),
                                            ),
                                            const SizedBox(height: 2),
                                            Text(
                                              item.artist,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: TextStyle(
                                                color: Colors.white.withValues(alpha: 0.56),
                                                fontSize: 12,
                                              ),
                                            ),
                                          ],
                                        ),
                                      ),
                                      IconButton(
                                        onPressed: () =>
                                            unawaited(controller.removeFromQueue(index)),
                                        icon: Icon(
                                          Icons.close_rounded,
                                          color: Colors.white.withValues(alpha: 0.35),
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                            );
                          },
                        ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _ProgressScrubBar extends StatelessWidget {
  const _ProgressScrubBar({
    super.key,
    required this.progressPercent,
    required this.onSeek,
  });

  final double progressPercent;
  final ValueChanged<double> onSeek;

  @override
  Widget build(BuildContext context) {
    final trackBg = Theme.of(context).brightness == Brightness.dark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.10);

    double calc(Offset global, BuildContext ctx) {
      final box = ctx.findRenderObject() as RenderBox?;
      if (box == null || box.size.width <= 0) return 0;
      final local = box.globalToLocal(global);
      return (local.dx / box.size.width).clamp(0.0, 1.0);
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (d) => onSeek(calc(d.globalPosition, context)),
          onHorizontalDragStart: (d) => onSeek(calc(d.globalPosition, context)),
          onHorizontalDragUpdate: (d) => onSeek(calc(d.globalPosition, context)),
          child: SizedBox(
            height: 14,
            child: Center(
              child: Stack(
                children: [
                  Container(height: 2, color: trackBg),
                  FractionallySizedBox(
                    widthFactor: progressPercent,
                    child: Container(
                      height: 2,
                      color: const Color(0xFF3B82F6),
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

class _SongSlot extends StatefulWidget {
  const _SongSlot({
    required this.song,
    required this.onTap,
  });

  final Song? song;
  final VoidCallback? onTap;

  @override
  State<_SongSlot> createState() => _SongSlotState();
}

class _SongSlotState extends State<_SongSlot> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final title = widget.song?.title ?? 'Not playing';
    final subtitle = widget.song?.artist ?? 'Select a song';
    final canExpand = widget.song != null;

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        MouseRegion(
          onEnter: (_) => setState(() => _hovering = true),
          onExit: (_) => setState(() => _hovering = false),
          cursor: canExpand ? SystemMouseCursors.click : SystemMouseCursors.basic,
          child: GestureDetector(
            onTap: widget.onTap,
            child: Stack(
              children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(6),
                  child: CoverArt(
                    width: 48,
                    height: 48,
                    coverHash: widget.song?.coverHash,
                    streamInfo: widget.song?.streamInfo,
                    size: CoverArtSize.small,
                    borderRadius: BorderRadius.circular(6),
                    placeholderIcon: PhosphorIcons.musicNotes(),
                    placeholderIconSize: 18,
                  ),
                ),
                if (_hovering && canExpand)
                  ClipRRect(
                    borderRadius: BorderRadius.circular(6),
                    child: Container(
                      width: 48,
                      height: 48,
                      color: Colors.black.withValues(alpha: 0.45),
                      child: Center(
                        child: Icon(
                          PhosphorIcons.arrowsOut(),
                          color: Colors.white,
                          size: 20,
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                ),
              ),
              const SizedBox(height: 2),
              Text(
                subtitle,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  fontSize: 12,
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _TransportCluster extends StatelessWidget {
  const _TransportCluster({
    required this.isCompact,
    required this.canControl,
    required this.isPlaying,
    required this.onPrevious,
    required this.onToggle,
    required this.onNext,
  });

  final bool isCompact;
  final bool canControl;
  final bool isPlaying;
  final VoidCallback onPrevious;
  final VoidCallback onToggle;
  final VoidCallback onNext;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (!isCompact)
          IconButton(
            onPressed: canControl ? onPrevious : null,
            tooltip: 'Previous',
            icon: Icon(PhosphorIcons.skipBack(PhosphorIconsStyle.fill), size: 22),
          ),
        IconButton.filled(
          onPressed: canControl ? onToggle : null,
          tooltip: isPlaying ? 'Pause' : 'Play',
          style: IconButton.styleFrom(
            backgroundColor: Theme.of(context).brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.12)
                : Colors.white,
            foregroundColor: Theme.of(context).brightness == Brightness.dark
                ? Colors.white
                : Colors.black,
            minimumSize: const Size(44, 44),
          ),
          icon: Icon(
            isPlaying
                ? PhosphorIcons.pause(PhosphorIconsStyle.fill)
                : PhosphorIcons.play(PhosphorIconsStyle.fill),
            size: 22,
          ),
        ),
        if (!isCompact)
          IconButton(
            onPressed: canControl ? onNext : null,
            tooltip: 'Next',
            icon: Icon(PhosphorIcons.skipForward(PhosphorIconsStyle.fill), size: 22),
          ),
      ],
    );
  }
}

class _RightActions extends StatelessWidget {
  const _RightActions({
    required this.isCompact,
    required this.queueCount,
    required this.volume,
    required this.volumeKey,
    required this.onQueue,
    required this.onSetVolume,
    required this.onToggleMute,
  });

  final bool isCompact;
  final int queueCount;
  final double volume;
  final GlobalKey volumeKey;
  final VoidCallback onQueue;
  final ValueChanged<double> onSetVolume;
  final VoidCallback onToggleMute;

  @override
  Widget build(BuildContext context) {
    final muted = volume <= 0.001;

    void _setFromGlobal(Offset global) {
      final ctx = volumeKey.currentContext;
      final box = ctx?.findRenderObject() as RenderBox?;
      if (box == null || box.size.width <= 0) return;
      final local = box.globalToLocal(global);
      final next = (local.dx / box.size.width).clamp(0.0, 1.0);
      onSetVolume(next);
    }

    Widget volumeBar() {
      return GestureDetector(
        key: volumeKey,
        behavior: HitTestBehavior.opaque,
        onTapDown: (d) => _setFromGlobal(d.globalPosition),
        onHorizontalDragStart: (d) => _setFromGlobal(d.globalPosition),
        onHorizontalDragUpdate: (d) => _setFromGlobal(d.globalPosition),
        child: SizedBox(
          width: 96,
          height: 18,
          child: Center(
            child: Stack(
              children: [
                Container(
                  height: 4,
                  decoration: BoxDecoration(
                    color: Theme.of(context).brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.12)
                        : Colors.black.withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(999),
                  ),
                ),
                FractionallySizedBox(
                  widthFactor: volume,
                  child: Container(
                    height: 4,
                    decoration: BoxDecoration(
                      color: Theme.of(context).brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.58)
                          : Colors.black.withValues(alpha: 0.45),
                      borderRadius: BorderRadius.circular(999),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    }

    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(
          onPressed: onQueue,
          tooltip: 'Queue',
          icon: Icon(
            PhosphorIcons.queue(),
            color: queueCount > 0 ? const Color(0xFF3B82F6) : null,
          ),
        ),
        if (!isCompact) ...[
          const SizedBox(width: 2),
          volumeBar(),
          const SizedBox(width: 2),
          IconButton(
            onPressed: onToggleMute,
            tooltip: muted ? 'Unmute' : 'Mute',
            icon: Icon(
              muted
                  ? PhosphorIcons.speakerSlash()
                  : (volume < 0.5
                      ? PhosphorIcons.speakerSimpleLow()
                      : PhosphorIcons.speakerHigh()),
            ),
          ),
        ],
      ],
    );
  }
}
