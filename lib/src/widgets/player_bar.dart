import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../models/models.dart';
import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';
import '../theme/design_tokens.dart';
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
    final tokens = ref.watch(bayinTokensProvider);

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

    return Container(
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
                isDark: tokens.isDark,
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
                        tokens: tokens,
                        onTap: song == null ? null : () => context.go('/player'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    _TransportCluster(
                      isCompact: isCompact,
                      canControl: canControl,
                      isPlaying: player.isPlaying,
                      isDark: tokens.isDark,
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
                        isDark: tokens.isDark,
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
              borderRadius: const BorderRadius.vertical(top: Radius.circular(FlatRadius.lg * 2)),
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
                    borderRadius: BorderRadius.circular(FlatRadius.circle),
                  ),
                ),
                const SizedBox(height: 12),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: FlatSpacing.md),
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
                              child: GestureDetector(
                                onTap: () {
                                  Navigator.of(context).pop();
                                  unawaited(controller.jumpTo(index));
                                },
                                child: Container(
                                  padding: const EdgeInsets.all(10),
                                  decoration: BoxDecoration(
                                    borderRadius: BorderRadius.circular(FlatRadius.md + 4),
                                    color: active
                                        ? Colors.white.withValues(alpha: 0.10)
                                        : Colors.transparent,
                                  ),
                                  child: Row(
                                    children: [
                                      ClipRRect(
                                        borderRadius: BorderRadius.circular(FlatRadius.md),
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
                                                    ? FlatColors.primaryDark
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
                                          PhosphorIcons.x(),
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
    required this.isDark,
    required this.onSeek,
  });

  final double progressPercent;
  final bool isDark;
  final ValueChanged<double> onSeek;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final trackBg = FlatColors.overlayOnDark(brightness);

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
                      color: FlatColors.primary(brightness),
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
    required this.tokens,
    required this.onTap,
  });

  final Song? song;
  final BayinTokens tokens;
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
                  color: widget.tokens.textSecondary,
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
    required this.isDark,
    required this.onPrevious,
    required this.onToggle,
    required this.onNext,
  });

  final bool isCompact;
  final bool canControl;
  final bool isPlaying;
  final bool isDark;
  final VoidCallback onPrevious;
  final VoidCallback onToggle;
  final VoidCallback onNext;

  @override
  Widget build(BuildContext context) {
    final playBg = isDark
        ? Colors.white.withValues(alpha: 0.12)
        : Colors.white;
    final playFg = isDark ? Colors.white : Colors.black;

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (!isCompact)
          IconButton(
            onPressed: canControl ? onPrevious : null,
            icon: Icon(PhosphorIcons.skipBack(PhosphorIconsStyle.fill), size: 22),
          ),
        Container(
          width: 44,
          height: 44,
          decoration: BoxDecoration(
            color: playBg,
            borderRadius: BorderRadius.circular(8),
          ),
          child: IconButton(
            onPressed: canControl ? onToggle : null,
            icon: Icon(
              isPlaying
                  ? PhosphorIcons.pause(PhosphorIconsStyle.fill)
                  : PhosphorIcons.play(PhosphorIconsStyle.fill),
              size: 22,
              color: playFg,
            ),
          ),
        ),
        if (!isCompact)
          IconButton(
            onPressed: canControl ? onNext : null,
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
    required this.isDark,
    required this.onQueue,
    required this.onSetVolume,
    required this.onToggleMute,
  });

  final bool isCompact;
  final int queueCount;
  final double volume;
  final GlobalKey volumeKey;
  final bool isDark;
  final VoidCallback onQueue;
  final ValueChanged<double> onSetVolume;
  final VoidCallback onToggleMute;

  @override
  Widget build(BuildContext context) {
    final muted = volume <= 0.001;

    void setFromGlobal(Offset global) {
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
        onTapDown: (d) => setFromGlobal(d.globalPosition),
        onHorizontalDragStart: (d) => setFromGlobal(d.globalPosition),
        onHorizontalDragUpdate: (d) => setFromGlobal(d.globalPosition),
        child: SizedBox(
          width: 96,
          height: 18,
          child: Center(
            child: Stack(
              children: [
                Container(
                  height: 4,
                  decoration: BoxDecoration(
                    color: isDark
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
                      color: isDark
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
          icon: Icon(
            PhosphorIcons.queue(),
            color: queueCount > 0 ? FlatColors.primary(Theme.of(context).brightness) : null,
          ),
        ),
        if (!isCompact) ...[
          const SizedBox(width: 2),
          volumeBar(),
          const SizedBox(width: 2),
          IconButton(
            onPressed: onToggleMute,
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
