import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../rust/rust_api.dart';
import '../../widgets/widgets.dart';

enum _PlayerView {
  cover('Cover'),
  lyrics('Lyrics'),
  split('Split');

  const _PlayerView(this.label);
  final String label;
}

class PlayerPage extends ConsumerStatefulWidget {
  const PlayerPage({super.key});

  @override
  ConsumerState<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends ConsumerState<PlayerPage> {
  _PlayerView _view = _PlayerView.cover;
  final ScrollController _lyricsScroll = ScrollController();
  int? _lastSyncedLyricIndex;
  bool _managedOrientation = false;

  @override
  void initState() {
    super.initState();
    _enableMobilePlayerOrientations();
  }

  @override
  void dispose() {
    _restorePreferredOrientations();
    _lyricsScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final player = ref.watch(playerControllerProvider);
    final controller = ref.read(playerControllerProvider.notifier);
    final song = player.currentSong;
    final lyricsAsync = ref.watch(lyricsLinesProvider);
    final activeLyricIndex = ref.watch(activeLyricIndexProvider);
    final fft = ref.watch(spectrumFrameProvider);
    final mode = ref.watch(spectrumModeProvider);
    final scheme = Theme.of(context).colorScheme;
    final modeController = ref.read(spectrumModeProvider.notifier);

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

    final lyricLines = lyricsAsync.valueOrNull ?? const <LyricLine>[];
    _syncLyricScroll(activeLyricIndex, lyricLines.length);

    return SafeArea(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1100),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(24, 20, 24, 24),
            child: Column(
              children: [
                _TopBar(
                  view: _view,
                  onViewChanged: (view) => setState(() => _view = view),
                ),
                const SizedBox(height: 12),
                Expanded(
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: scheme.surface.withValues(alpha: 0.84),
                      borderRadius: BorderRadius.circular(24),
                      border: Border.all(
                        color: scheme.outlineVariant.withValues(alpha: 0.35),
                      ),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.all(18),
                      child: _PlayerBody(
                        view: _view,
                        song: song,
                        fft: fft,
                        mode: mode,
                        onModeChanged: (next) => modeController.state = next,
                        lyricsAsync: lyricsAsync,
                        lyricLines: lyricLines,
                        activeLyricIndex: activeLyricIndex,
                        lyricsScroll: _lyricsScroll,
                        positionMs: (player.positionSecs * 1000).round(),
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 12),
                _PlaybackControls(
                  state: player,
                  controller: controller,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  void _syncLyricScroll(int? activeIndex, int lineCount) {
    if (!_lyricsScroll.hasClients || activeIndex == null || lineCount == 0) {
      return;
    }
    if (_lastSyncedLyricIndex == activeIndex) {
      return;
    }
    _lastSyncedLyricIndex = activeIndex;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_lyricsScroll.hasClients) {
        return;
      }
      const itemExtent = 68.0;
      final target = (activeIndex * itemExtent - 140).clamp(
        0.0,
        _lyricsScroll.position.maxScrollExtent,
      );
      _lyricsScroll.animateTo(
        target,
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeOut,
      );
    });
  }

  void _enableMobilePlayerOrientations() {
    if (kIsWeb) return;
    if (!(Platform.isAndroid || Platform.isIOS)) return;
    _managedOrientation = true;
    unawaited(SystemChrome.setPreferredOrientations(const [
      DeviceOrientation.portraitUp,
      DeviceOrientation.portraitDown,
      DeviceOrientation.landscapeLeft,
      DeviceOrientation.landscapeRight,
    ]));
  }

  void _restorePreferredOrientations() {
    if (!_managedOrientation) {
      return;
    }
    unawaited(SystemChrome.setPreferredOrientations(const []));
    _managedOrientation = false;
  }
}

class _TopBar extends StatelessWidget {
  const _TopBar({
    required this.view,
    required this.onViewChanged,
  });

  final _PlayerView view;
  final ValueChanged<_PlayerView> onViewChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
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
        const SizedBox(width: 8),
        SegmentedButton<_PlayerView>(
          segments: [
            for (final item in _PlayerView.values)
              ButtonSegment<_PlayerView>(
                value: item,
                label: Text(item.label),
              ),
          ],
          selected: <_PlayerView>{view},
          onSelectionChanged: (selection) {
            final next = selection.isEmpty ? null : selection.first;
            if (next != null) {
              onViewChanged(next);
            }
          },
        ),
      ],
    );
  }
}

class _PlayerBody extends StatelessWidget {
  const _PlayerBody({
    required this.view,
    required this.song,
    required this.fft,
    required this.mode,
    required this.onModeChanged,
    required this.lyricsAsync,
    required this.lyricLines,
    required this.activeLyricIndex,
    required this.lyricsScroll,
    required this.positionMs,
  });

  final _PlayerView view;
  final Song song;
  final RustFftSnapshot fft;
  final SpectrumMode mode;
  final ValueChanged<SpectrumMode> onModeChanged;
  final AsyncValue<List<LyricLine>> lyricsAsync;
  final List<LyricLine> lyricLines;
  final int? activeLyricIndex;
  final ScrollController lyricsScroll;
  final int positionMs;

  @override
  Widget build(BuildContext context) {
    switch (view) {
      case _PlayerView.cover:
        return Column(
          children: [
            Expanded(
              child: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 520),
                  child: PlayerStage(song: song, mode: mode, fft: fft),
                ),
              ),
            ),
            const SizedBox(height: 12),
            _ModePicker(mode: mode, onModeChanged: onModeChanged),
          ],
        );
      case _PlayerView.lyrics:
        return _LyricsPanel(
          lyricsAsync: lyricsAsync,
          lyricLines: lyricLines,
          activeLyricIndex: activeLyricIndex,
          lyricsScroll: lyricsScroll,
          positionMs: positionMs,
        );
      case _PlayerView.split:
        return LayoutBuilder(
          builder: (context, constraints) {
            if (constraints.maxWidth < 860) {
              return Column(
                children: [
                  Expanded(
                    child: PlayerStage(song: song, mode: mode, fft: fft),
                  ),
                  const SizedBox(height: 10),
                  _ModePicker(mode: mode, onModeChanged: onModeChanged),
                  const SizedBox(height: 10),
                  Expanded(
                    child: _LyricsPanel(
                      lyricsAsync: lyricsAsync,
                      lyricLines: lyricLines,
                      activeLyricIndex: activeLyricIndex,
                      lyricsScroll: lyricsScroll,
                      positionMs: positionMs,
                    ),
                  ),
                ],
              );
            }
            return Row(
              children: [
                Expanded(
                  child: Column(
                    children: [
                      Expanded(
                        child: PlayerStage(song: song, mode: mode, fft: fft),
                      ),
                      const SizedBox(height: 12),
                      _ModePicker(mode: mode, onModeChanged: onModeChanged),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: _LyricsPanel(
                    lyricsAsync: lyricsAsync,
                    lyricLines: lyricLines,
                    activeLyricIndex: activeLyricIndex,
                    lyricsScroll: lyricsScroll,
                    positionMs: positionMs,
                  ),
                ),
              ],
            );
          },
        );
    }
  }
}

class _ModePicker extends StatelessWidget {
  const _ModePicker({
    required this.mode,
    required this.onModeChanged,
  });

  final SpectrumMode mode;
  final ValueChanged<SpectrumMode> onModeChanged;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          for (final item in SpectrumMode.values)
            Padding(
              padding: const EdgeInsets.only(right: 8),
              child: ChoiceChip(
                label: Text(_label(item)),
                selected: mode == item,
                onSelected: (_) => onModeChanged(item),
              ),
            ),
        ],
      ),
    );
  }

  String _label(SpectrumMode mode) {
    switch (mode) {
      case SpectrumMode.wave:
        return 'Wave';
      case SpectrumMode.godRing:
        return 'God Ring';
      case SpectrumMode.diffusionRing:
        return 'Diffusion';
      case SpectrumMode.trippyRipple:
        return 'Trippy';
      case SpectrumMode.attachmentRing:
        return 'Attachment';
      case SpectrumMode.rotatingCover:
        return 'Rotating';
      case SpectrumMode.bessel:
        return 'Bessel';
      case SpectrumMode.columnar:
        return 'Columnar';
    }
  }
}

class _LyricsPanel extends StatelessWidget {
  const _LyricsPanel({
    required this.lyricsAsync,
    required this.lyricLines,
    required this.activeLyricIndex,
    required this.lyricsScroll,
    required this.positionMs,
  });

  final AsyncValue<List<LyricLine>> lyricsAsync;
  final List<LyricLine> lyricLines;
  final int? activeLyricIndex;
  final ScrollController lyricsScroll;
  final int positionMs;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    if (lyricsAsync.isLoading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (lyricLines.isEmpty) {
      return Center(
        child: Text(
          'No lyrics found for current song.',
          style: TextStyle(color: scheme.onSurfaceVariant),
        ),
      );
    }

    return ListView.builder(
      controller: lyricsScroll,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
      itemCount: lyricLines.length,
      itemExtent: 68,
      itemBuilder: (context, index) {
        final line = lyricLines[index];
        final active = index == activeLyricIndex;
        return AnimatedOpacity(
          duration: const Duration(milliseconds: 160),
          opacity: active ? 1 : 0.68,
          child: Center(
            child: KaraokeLine(
              line: line,
              positionMs: positionMs,
              active: active,
            ),
          ),
        );
      },
    );
  }
}

class _PlaybackControls extends StatelessWidget {
  const _PlaybackControls({
    required this.state,
    required this.controller,
  });

  final PlayerControllerState state;
  final PlayerController controller;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Row(
      children: [
        Expanded(
          child: Row(
            children: [
              IconButton(
                onPressed: controller.previous,
                icon: Icon(PhosphorIcons.skipBack()),
                iconSize: 24,
                tooltip: 'Previous',
              ),
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
              IconButton(
                onPressed: controller.next,
                icon: Icon(PhosphorIcons.skipForward()),
                iconSize: 24,
                tooltip: 'Next',
              ),
              const SizedBox(width: 12),
              Text(
                '${_formatDuration(state.positionSecs)} / ${_formatDuration(state.durationSecs)}',
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
            ],
          ),
        ),
        SizedBox(
          width: 180,
          child: Row(
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
            ],
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
