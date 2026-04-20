import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';

class PlayerPage extends ConsumerWidget {
  const PlayerPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final player = ref.watch(playerControllerProvider);
    final controller = ref.read(playerControllerProvider.notifier);
    final song = player.currentSong;
    final scheme = Theme.of(context).colorScheme;

    if (song == null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.musicNotes(),
              size: 56,
              color: scheme.onSurfaceVariant,
            ),
            const SizedBox(height: 12),
            Text(
              'Nothing is playing',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 8),
            Text(
              'Pick a song from your library to start playback.',
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          ],
        ),
      );
    }

    return SafeArea(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 980),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(24, 20, 24, 24),
            child: LayoutBuilder(
              builder: (context, constraints) {
                final wide = constraints.maxWidth >= 760;
                return SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      IconButton(
                        onPressed: () {
                          if (context.canPop()) {
                            context.pop();
                          } else {
                            context.go('/');
                          }
                        },
                        icon: Icon(PhosphorIcons.arrowLeft()),
                        tooltip: 'Back',
                      ),
                      const SizedBox(height: 8),
                      Container(
                        padding: const EdgeInsets.all(24),
                        decoration: BoxDecoration(
                          color: scheme.surface.withValues(alpha: 0.8),
                          borderRadius: BorderRadius.circular(28),
                          border: Border.all(
                            color: scheme.outlineVariant.withValues(alpha: 0.4),
                          ),
                        ),
                        child: wide
                            ? Row(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  _CoverPanel(song: song),
                                  const SizedBox(width: 24),
                                  Expanded(
                                    child: _DetailsPanel(
                                      state: player,
                                      controller: controller,
                                    ),
                                  ),
                                ],
                              )
                            : Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: [
                                  Align(
                                    alignment: Alignment.center,
                                    child: _CoverPanel(song: song),
                                  ),
                                  const SizedBox(height: 20),
                                  _DetailsPanel(
                                    state: player,
                                    controller: controller,
                                  ),
                                ],
                              ),
                      ),
                      const SizedBox(height: 20),
                      _QueuePanel(
                        state: player,
                        onJumpTo: controller.jumpTo,
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _CoverPanel extends StatelessWidget {
  const _CoverPanel({required this.song});

  final Song song;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: 280,
      height: 280,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(28),
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            scheme.primary.withValues(alpha: 0.25),
            scheme.secondary.withValues(alpha: 0.2),
            scheme.surfaceContainerHighest,
          ],
        ),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(28),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
          child: Center(
            child: Icon(
              PhosphorIcons.musicNote(PhosphorIconsStyle.fill),
              size: 72,
              color: scheme.onSurfaceVariant,
            ),
          ),
        ),
      ),
    );
  }
}

class _DetailsPanel extends StatelessWidget {
  const _DetailsPanel({
    required this.state,
    required this.controller,
  });

  final PlayerControllerState state;
  final PlayerController controller;

  @override
  Widget build(BuildContext context) {
    final song = state.currentSong!;
    final scheme = Theme.of(context).colorScheme;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          song.title,
          style: Theme.of(context).textTheme.headlineSmall,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
        ),
        const SizedBox(height: 8),
        Text(
          '${song.artist} - ${song.album}',
          style: TextStyle(
            fontSize: 15,
            color: scheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 20),
        _ModeRow(
          current: state.mode,
          onSelected: controller.setPlayMode,
        ),
        const SizedBox(height: 20),
        _PageProgressSlider(
          positionSecs: state.positionSecs,
          durationSecs: state.durationSecs,
        ),
        const SizedBox(height: 12),
        Row(
          children: [
            IconButton(
              onPressed: controller.previous,
              icon: Icon(PhosphorIcons.skipBack()),
              iconSize: 24,
              tooltip: 'Previous',
            ),
            const SizedBox(width: 8),
            IconButton.filled(
              onPressed: controller.togglePlayPause,
              icon: Icon(
                state.isPlaying
                    ? PhosphorIcons.pause(PhosphorIconsStyle.fill)
                    : PhosphorIcons.play(PhosphorIconsStyle.fill),
              ),
              iconSize: 22,
              tooltip: state.isPlaying ? 'Pause' : 'Play',
            ),
            const SizedBox(width: 8),
            IconButton(
              onPressed: controller.next,
              icon: Icon(PhosphorIcons.skipForward()),
              iconSize: 24,
              tooltip: 'Next',
            ),
            const SizedBox(width: 8),
            OutlinedButton.icon(
              onPressed: controller.stop,
              icon: Icon(PhosphorIcons.stop()),
              label: const Text('Stop'),
            ),
          ],
        ),
        const SizedBox(height: 20),
        Row(
          children: [
            Icon(
              state.volume <= 0.01
                  ? PhosphorIcons.speakerSlash()
                  : PhosphorIcons.speakerHigh(),
              size: 18,
              color: scheme.onSurfaceVariant,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Slider(
                value: state.volume.clamp(0.0, 1.0),
                min: 0,
                max: 1,
                onChanged: controller.setVolume,
              ),
            ),
            SizedBox(
              width: 44,
              child: Text(
                '${(state.volume * 100).round()}%',
                textAlign: TextAlign.right,
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
            ),
          ],
        ),
        if (state.error != null) ...[
          const SizedBox(height: 12),
          Text(
            state.error!,
            style: TextStyle(color: scheme.error),
          ),
        ],
      ],
    );
  }
}

class _ModeRow extends StatelessWidget {
  const _ModeRow({
    required this.current,
    required this.onSelected,
  });

  final PlayMode current;
  final ValueChanged<PlayMode> onSelected;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        _ModeChip(
          label: 'Sequence',
          selected: current == PlayMode.sequence,
          onTap: () => onSelected(PlayMode.sequence),
        ),
        _ModeChip(
          label: 'Shuffle',
          selected: current == PlayMode.shuffle,
          onTap: () => onSelected(PlayMode.shuffle),
        ),
        _ModeChip(
          label: 'Repeat one',
          selected: current == PlayMode.repeatOne,
          onTap: () => onSelected(PlayMode.repeatOne),
        ),
      ],
    );
  }
}

class _ModeChip extends StatelessWidget {
  const _ModeChip({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return ChoiceChip(
      label: Text(label),
      selected: selected,
      onSelected: (_) => onTap(),
    );
  }
}

class _PageProgressSlider extends ConsumerStatefulWidget {
  const _PageProgressSlider({
    required this.positionSecs,
    required this.durationSecs,
  });

  final double positionSecs;
  final double durationSecs;

  @override
  ConsumerState<_PageProgressSlider> createState() => _PageProgressSliderState();
}

class _PageProgressSliderState extends ConsumerState<_PageProgressSlider> {
  double? _dragValue;

  @override
  Widget build(BuildContext context) {
    final max = widget.durationSecs > 0 ? widget.durationSecs : 1.0;
    final value = (_dragValue ?? widget.positionSecs).clamp(0.0, max);
    final scheme = Theme.of(context).colorScheme;

    return Column(
      children: [
        Slider(
          value: value,
          min: 0,
          max: max,
          onChanged: (next) => setState(() => _dragValue = next),
          onChangeEnd: (next) {
            setState(() => _dragValue = null);
            ref.read(playerControllerProvider.notifier).seek(next);
          },
        ),
        Row(
          children: [
            Text(
              _formatDuration(widget.positionSecs),
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
            const Spacer(),
            Text(
              _formatDuration(widget.durationSecs),
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          ],
        ),
      ],
    );
  }
}

class _QueuePanel extends StatelessWidget {
  const _QueuePanel({
    required this.state,
    required this.onJumpTo,
  });

  final PlayerControllerState state;
  final Future<void> Function(int index) onJumpTo;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: scheme.surface.withValues(alpha: 0.8),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(
          color: scheme.outlineVariant.withValues(alpha: 0.4),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Queue',
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: 12),
          if (state.queue.isEmpty)
            Text(
              'Your playback queue is empty.',
              style: TextStyle(color: scheme.onSurfaceVariant),
            )
          else
            ListView.separated(
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              itemCount: state.queue.length,
              separatorBuilder: (_, _) => Divider(
                height: 1,
                color: scheme.outlineVariant.withValues(alpha: 0.3),
              ),
              itemBuilder: (context, index) {
                final song = state.queue[index];
                final isCurrent = index == state.currentIndex;
                return ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: Icon(
                    isCurrent
                        ? PhosphorIcons.waveform(PhosphorIconsStyle.fill)
                        : PhosphorIcons.musicNote(),
                    color: isCurrent ? scheme.primary : scheme.onSurfaceVariant,
                  ),
                  title: Text(
                    song.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: Text(
                    '${song.artist} - ${song.album}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Text(_formatDuration(song.duration)),
                  selected: isCurrent,
                  onTap: () => onJumpTo(index),
                );
              },
            ),
        ],
      ),
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
