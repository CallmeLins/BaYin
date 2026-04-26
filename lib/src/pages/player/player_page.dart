import 'dart:async';
import 'dart:io';
import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../rust/rust_api.dart';
import '../../services/settings_service.dart';
import '../../widgets/widgets.dart';
import '../../widgets/spectrum/spectrum_painter.dart';

const int _userScrollTimeoutMs = 3000;
const String _desktopSplitViewStorageKey = 'bayin_player_desktop_split_view';

class PlayerPage extends ConsumerStatefulWidget {
  const PlayerPage({super.key});

  @override
  ConsumerState<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends ConsumerState<PlayerPage> {
  final ScrollController _lyricsScroll = ScrollController();

  bool _showLyrics = false;
  bool _desktopSplitView = false;
  bool _managedOrientation = false;
  bool _userScrollingLyrics = false;
  bool _needsLyricSyncOnShow = false;

  int? _lastSyncedLyricIndex;
  Timer? _userScrollResumeTimer;
  double? _dragProgressSecs;
  String? _dragSongId;

  @override
  void initState() {
    super.initState();
    _enableMobilePlayerOrientations();
    _restoreDesktopSplitView();
  }

  @override
  void dispose() {
    _restorePreferredOrientations();
    _userScrollResumeTimer?.cancel();
    _lyricsScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final player = ref.watch(playerControllerProvider);
    final controller = ref.read(playerControllerProvider.notifier);
    final appSettings = ref.watch(appSettingsProvider);
    final appSettingsController = ref.read(appSettingsProvider.notifier);
    final responsive = ref.watch(responsiveLayoutProvider);
    final lyricsAsync = ref.watch(lyricsLinesProvider);
    final fft = ref.watch(spectrumFrameProvider);
    final spectrumMode = ref.watch(spectrumModeProvider);
    final spectrumModeController = ref.read(spectrumModeProvider.notifier);
    final song = player.currentSong;

    if (song == null) {
      return _EmptyPlayer(
        onBack: () {
          if (context.canPop()) {
            context.pop();
          } else {
            context.go('/');
          }
        },
      );
    }

    if (_dragSongId != song.id) {
      _dragSongId = song.id;
      _dragProgressSecs = null;
    }

    final lyricLines = lyricsAsync.valueOrNull ?? const <LyricLine>[];
    final adjustedPositionMs = ((player.positionSecs * 1000).round() +
            appSettings.lyricsOffsetMs)
        .clamp(0, 1 << 30)
        .toInt();
    final activeLyricIndex = _findActiveLyricIndex(lyricLines, adjustedPositionMs);

    final isPhoneLandscapeControls =
        responsive.playerMode == PlayerMode.phoneLandscapeControls;
    final showDesktopSplitToggle =
        responsive.playerMode == PlayerMode.toggleSplit;

    final pureModeFeatureEnabled =
        appSettings.proEnabled && appSettings.proPureModeEnabled;
    final pureModeActive = pureModeFeatureEnabled && appSettings.pureModeEnabled;
    final proSpectrumActive =
        appSettings.proEnabled && appSettings.proColorSpectrumEnabled;

    if (!pureModeFeatureEnabled && appSettings.pureModeEnabled) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        unawaited(appSettingsController.setPureModeEnabled(false));
      });
    }

    if (isPhoneLandscapeControls && _showLyrics) {
      _deferSetLyricsVisible(false);
    }
    if (pureModeActive && _showLyrics) {
      _deferSetLyricsVisible(false);
    }

    final splitViewActive = showDesktopSplitToggle &&
        !pureModeActive &&
        _desktopSplitView &&
        !isPhoneLandscapeControls;

    final ringLayoutActive = proSpectrumActive &&
        (spectrumMode == SpectrumMode.godRing ||
            spectrumMode == SpectrumMode.diffusionRing ||
            spectrumMode == SpectrumMode.attachmentRing ||
            spectrumMode == SpectrumMode.rotatingCover ||
            spectrumMode == SpectrumMode.trippyRipple);
    final showRingSpectrum = appSettings.visualizerEnabled && ringLayoutActive;
    final spectrumIsBottom =
        spectrumMode == SpectrumMode.bessel || spectrumMode == SpectrumMode.columnar;
    final showBottomSpectrum = appSettings.visualizerEnabled;
    final bottomSpectrumMode =
        (proSpectrumActive && spectrumIsBottom)
            ? spectrumMode
            : SpectrumMode.wave;
    final stageMode = showRingSpectrum ? spectrumMode : SpectrumMode.wave;

    final showLyricsPanel = splitViewActive || _showLyrics;
    if (showLyricsPanel) {
      _syncLyricScroll(activeLyricIndex, lyricLines.length);
    }

    final effectiveDuration =
        (player.durationSecs > 0 ? player.durationSecs : song.duration)
            .clamp(0.0, 1e9);
    final sliderProgress =
        (_dragProgressSecs ?? player.positionSecs).clamp(0.0, effectiveDuration);

    return Stack(
      fit: StackFit.expand,
      children: [
        _PlayerBackground(song: song),
        Positioned.fill(
          child: DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [
                  Colors.black.withValues(alpha: 0.52),
                  Colors.black.withValues(alpha: 0.65),
                ],
              ),
            ),
          ),
        ),
        if (showBottomSpectrum)
          _BottomSpectrumLayer(
            fft: fft,
            mode: bottomSpectrumMode,
          ),
        SafeArea(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(14, 10, 14, 14),
            child: Column(
              children: [
                if (!isPhoneLandscapeControls)
                  _PlayerTopBar(
                    onBack: () {
                      if (context.canPop()) {
                        context.pop();
                      } else {
                        context.go('/');
                      }
                    },
                    onOpenQueue: () => _openQueueSheet(context, player, controller),
                    onOpenViewSettings: () => _openViewSettingsSheet(
                      context,
                      showDesktopSplitToggle: showDesktopSplitToggle,
                      splitViewEnabled: _desktopSplitView,
                      pureModeFeatureEnabled: pureModeFeatureEnabled,
                      pureModeActive: pureModeActive,
                      proSpectrumActive: proSpectrumActive,
                      mode: spectrumMode,
                      onSplitViewChanged: _setDesktopSplitView,
                      onPureModeChanged: (value) {
                        unawaited(appSettingsController.setPureModeEnabled(value));
                      },
                      onSpectrumModeChanged: (value) {
                        spectrumModeController.state = value;
                      },
                    ),
                  ),
                if (!isPhoneLandscapeControls) const SizedBox(height: 10),
                Expanded(
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 220),
                    switchInCurve: Curves.easeOutCubic,
                    switchOutCurve: Curves.easeInCubic,
                    child: isPhoneLandscapeControls
                        ? _PhoneLandscapeBody(
                            key: const ValueKey('landscape'),
                            song: song,
                            player: player,
                            controller: controller,
                            fft: fft,
                            mode: stageMode,
                            showRingSpectrum: showRingSpectrum,
                            ringLayoutActive: ringLayoutActive,
                            sliderProgress: sliderProgress,
                            effectiveDuration: effectiveDuration,
                            onProgressChanged: (value) =>
                                setState(() => _dragProgressSecs = value),
                            onProgressChangeEnd: (value) {
                              setState(() => _dragProgressSecs = null);
                              unawaited(controller.seek(value));
                            },
                            onTogglePlayMode: () =>
                                _togglePlayMode(controller, player.mode),
                            onOpenSongMenu: () =>
                                SongMenu.show(context, song: song),
                            onOpenQueue: () =>
                                _openQueueSheet(context, player, controller),
                            onBack: () {
                              if (context.canPop()) {
                                context.pop();
                              } else {
                                context.go('/');
                              }
                            },
                            onOpenViewSettings: () => _openViewSettingsSheet(
                              context,
                              showDesktopSplitToggle: showDesktopSplitToggle,
                              splitViewEnabled: _desktopSplitView,
                              pureModeFeatureEnabled: pureModeFeatureEnabled,
                              pureModeActive: pureModeActive,
                              proSpectrumActive: proSpectrumActive,
                              mode: spectrumMode,
                              onSplitViewChanged: _setDesktopSplitView,
                              onPureModeChanged: (value) {
                                unawaited(
                                  appSettingsController.setPureModeEnabled(value),
                                );
                              },
                              onSpectrumModeChanged: (value) {
                                spectrumModeController.state = value;
                              },
                            ),
                          )
                        : splitViewActive
                            ? _SplitBody(
                                key: const ValueKey('split'),
                                song: song,
                                player: player,
                                controller: controller,
                                fft: fft,
                                mode: stageMode,
                                showRingSpectrum: showRingSpectrum,
                                ringLayoutActive: ringLayoutActive,
                                lyricsAsync: lyricsAsync,
                                lyricLines: lyricLines,
                                activeLyricIndex: activeLyricIndex,
                                adjustedPositionMs: adjustedPositionMs,
                                sliderProgress: sliderProgress,
                                effectiveDuration: effectiveDuration,
                                lyricFontSize: appSettings.lyricsFontSize,
                                lyricAlign: _lyricsAlign(appSettings.lyricsPosition),
                                lyricSelectable: appSettings.lyricsSelectable,
                                lyricAutoBlur: appSettings.lyricsAutoBlur,
                                lyricWordByWordAnimation:
                                    appSettings.lyricsWordByWordAnimation,
                                lyricsScroll: _lyricsScroll,
                                onLyricTap: (lineStartMs) => _seekLyricLine(
                                  lineStartMs,
                                  appSettings.lyricsOffsetMs,
                                  controller,
                                ),
                                onLyricsUserScroll: _markLyricsUserScrolling,
                                onProgressChanged: (value) =>
                                    setState(() => _dragProgressSecs = value),
                                onProgressChangeEnd: (value) {
                                  setState(() => _dragProgressSecs = null);
                                  unawaited(controller.seek(value));
                                },
                                onTogglePlayMode: () =>
                                    _togglePlayMode(controller, player.mode),
                                onOpenSongMenu: () =>
                                    SongMenu.show(context, song: song),
                              )
                            : _SingleBody(
                                key: const ValueKey('single'),
                                showLyrics: _showLyrics,
                                pureModeActive: pureModeActive,
                                song: song,
                                player: player,
                                controller: controller,
                                fft: fft,
                                mode: stageMode,
                                showRingSpectrum: showRingSpectrum,
                                ringLayoutActive: ringLayoutActive,
                                lyricsAsync: lyricsAsync,
                                lyricLines: lyricLines,
                                activeLyricIndex: activeLyricIndex,
                                adjustedPositionMs: adjustedPositionMs,
                                sliderProgress: sliderProgress,
                                effectiveDuration: effectiveDuration,
                                lyricFontSize: appSettings.lyricsFontSize,
                                lyricAlign: _lyricsAlign(appSettings.lyricsPosition),
                                lyricSelectable: appSettings.lyricsSelectable,
                                lyricAutoBlur: appSettings.lyricsAutoBlur,
                                lyricWordByWordAnimation:
                                    appSettings.lyricsWordByWordAnimation,
                                lyricsScroll: _lyricsScroll,
                                onToggleLyrics: () => _setLyricsVisible(!_showLyrics),
                                onLyricTap: (lineStartMs) => _seekLyricLine(
                                  lineStartMs,
                                  appSettings.lyricsOffsetMs,
                                  controller,
                                ),
                                onLyricsUserScroll: _markLyricsUserScrolling,
                                onProgressChanged: (value) =>
                                    setState(() => _dragProgressSecs = value),
                                onProgressChangeEnd: (value) {
                                  setState(() => _dragProgressSecs = null);
                                  unawaited(controller.seek(value));
                                },
                                onTogglePlayMode: () =>
                                    _togglePlayMode(controller, player.mode),
                                onOpenSongMenu: () =>
                                    SongMenu.show(context, song: song),
                              ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _restoreDesktopSplitView() async {
    try {
      final stored =
          SettingsService.instance.readBool(_desktopSplitViewStorageKey) ?? false;
      if (!mounted) {
        return;
      }
      setState(() => _desktopSplitView = stored);
    } catch (_) {
      // Keep view state optional.
    }
  }

  void _setDesktopSplitView(bool enabled) {
    setState(() => _desktopSplitView = enabled);
    unawaited(
      SettingsService.instance.writeBool(_desktopSplitViewStorageKey, enabled),
    );
    if (!enabled && _showLyrics) {
      _setLyricsVisible(false);
    }
  }

  void _setLyricsVisible(bool value) {
    setState(() {
      _showLyrics = value;
      if (value) {
        _needsLyricSyncOnShow = true;
        _userScrollingLyrics = false;
      }
    });
  }

  void _deferSetLyricsVisible(bool value) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _setLyricsVisible(value);
    });
  }

  void _markLyricsUserScrolling() {
    if (!_userScrollingLyrics) {
      setState(() => _userScrollingLyrics = true);
    }
    _userScrollResumeTimer?.cancel();
    _userScrollResumeTimer = Timer(
      const Duration(milliseconds: _userScrollTimeoutMs),
      () {
        if (!mounted) {
          return;
        }
        setState(() {
          _userScrollingLyrics = false;
          _needsLyricSyncOnShow = true;
        });
      },
    );
  }

  void _syncLyricScroll(int activeLyricIndex, int lineCount) {
    if (!_lyricsScroll.hasClients || lineCount == 0 || _userScrollingLyrics) {
      return;
    }

    final targetIndex = activeLyricIndex < 0 ? 0 : activeLyricIndex;
    if (_lastSyncedLyricIndex == targetIndex && !_needsLyricSyncOnShow) {
      return;
    }

    _lastSyncedLyricIndex = targetIndex;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_lyricsScroll.hasClients) {
        return;
      }
      const itemExtent = 78.0;
      final viewport = _lyricsScroll.position.viewportDimension;
      final target = (targetIndex * itemExtent - (viewport - itemExtent) / 2)
          .clamp(0.0, _lyricsScroll.position.maxScrollExtent);

      if (_needsLyricSyncOnShow) {
        _lyricsScroll.jumpTo(target);
      } else {
        _lyricsScroll.animateTo(
          target,
          duration: const Duration(milliseconds: 420),
          curve: Curves.easeOutCubic,
        );
      }
      _needsLyricSyncOnShow = false;
    });
  }

  int _findActiveLyricIndex(List<LyricLine> lines, int adjustedPositionMs) {
    if (lines.isEmpty) {
      return -1;
    }
    for (var i = lines.length - 1; i >= 0; i--) {
      if (adjustedPositionMs >= lines[i].startMs) {
        return i;
      }
    }
    return -1;
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
      barrierColor: Colors.black.withValues(alpha: 0.35),
      builder: (_) {
        return _QueueSheet(
          player: player,
          onJumpTo: (index) {
            Navigator.of(context).maybePop();
            unawaited(controller.jumpTo(index));
          },
          onRemove: (index) {
            unawaited(controller.removeFromQueue(index));
          },
          onClear: controller.clearQueue,
        );
      },
    );
  }

  Future<void> _openViewSettingsSheet(
    BuildContext context, {
    required bool showDesktopSplitToggle,
    required bool splitViewEnabled,
    required bool pureModeFeatureEnabled,
    required bool pureModeActive,
    required bool proSpectrumActive,
    required SpectrumMode mode,
    required ValueChanged<bool> onSplitViewChanged,
    required ValueChanged<bool> onPureModeChanged,
    required ValueChanged<SpectrumMode> onSpectrumModeChanged,
  }) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      barrierColor: Colors.black.withValues(alpha: 0.35),
      builder: (sheetContext) {
        return _ViewSettingsSheet(
          showDesktopSplitToggle: showDesktopSplitToggle,
          splitViewEnabled: splitViewEnabled,
          pureModeFeatureEnabled: pureModeFeatureEnabled,
          pureModeActive: pureModeActive,
          proSpectrumActive: proSpectrumActive,
          mode: mode,
          onSplitViewChanged: (value) {
            onSplitViewChanged(value);
          },
          onPureModeChanged: onPureModeChanged,
          onSpectrumModeChanged: onSpectrumModeChanged,
        );
      },
    );
  }

  void _togglePlayMode(PlayerController controller, PlayMode currentMode) {
    final modes = <PlayMode>[
      PlayMode.sequence,
      PlayMode.shuffle,
      PlayMode.repeatOne,
    ];
    final currentIndex = modes.indexOf(currentMode);
    final next = modes[(currentIndex + 1) % modes.length];
    controller.setPlayMode(next);
  }

  void _seekLyricLine(
    int lineStartMs,
    int lyricsOffsetMs,
    PlayerController controller,
  ) {
    final targetSecs = ((lineStartMs - lyricsOffsetMs).clamp(0, 1 << 30)) / 1000.0;
    unawaited(controller.seek(targetSecs));
  }

  void _enableMobilePlayerOrientations() {
    if (kIsWeb) return;
    if (!(Platform.isAndroid || Platform.isIOS)) return;
    _managedOrientation = true;
    unawaited(
      SystemChrome.setPreferredOrientations(const [
        DeviceOrientation.portraitUp,
        DeviceOrientation.portraitDown,
        DeviceOrientation.landscapeLeft,
        DeviceOrientation.landscapeRight,
      ]),
    );
  }

  void _restorePreferredOrientations() {
    if (!_managedOrientation) {
      return;
    }
    unawaited(SystemChrome.setPreferredOrientations(const []));
    _managedOrientation = false;
  }
}

class _EmptyPlayer extends StatelessWidget {
  const _EmptyPlayer({required this.onBack});

  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
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
          const SizedBox(height: 16),
          FilledButton.icon(
            onPressed: onBack,
            icon: const Icon(Icons.arrow_back),
            label: const Text('Back'),
          ),
        ],
      ),
    );
  }
}

class _PlayerBackground extends StatelessWidget {
  const _PlayerBackground({required this.song});

  final Song song;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        final height = constraints.maxHeight;
        return Stack(
          fit: StackFit.expand,
          children: [
            ImageFiltered(
              imageFilter: ImageFilter.blur(sigmaX: 34, sigmaY: 34),
              child: CoverArt(
                width: width,
                height: height,
                coverHash: song.coverHash,
                streamInfo: song.streamInfo,
                size: CoverArtSize.mid,
                fit: BoxFit.cover,
              ),
            ),
            Positioned.fill(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  gradient: RadialGradient(
                    center: const Alignment(0, -0.2),
                    radius: 1.3,
                    colors: [
                      Colors.white.withValues(alpha: 0.06),
                      Colors.transparent,
                    ],
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _BottomSpectrumLayer extends StatelessWidget {
  const _BottomSpectrumLayer({
    required this.fft,
    required this.mode,
  });

  final RustFftSnapshot fft;
  final SpectrumMode mode;

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: Align(
        alignment: Alignment.bottomCenter,
        child: SizedBox(
          height: 240,
          child: Stack(
            fit: StackFit.expand,
            children: [
              DecoratedBox(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [
                      Colors.transparent,
                      Colors.white.withValues(alpha: 0.08),
                    ],
                  ),
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(8, 48, 8, 0),
                child: CustomPaint(
                  painter: SpectrumPainter(
                    frequency: fft.frequency,
                    waveform: fft.waveform,
                    mode: mode,
                    color: Colors.white.withValues(alpha: 0.9),
                  ),
                  child: const SizedBox.expand(),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PlayerTopBar extends StatelessWidget {
  const _PlayerTopBar({
    required this.onBack,
    required this.onOpenQueue,
    required this.onOpenViewSettings,
  });

  final VoidCallback onBack;
  final VoidCallback onOpenQueue;
  final VoidCallback onOpenViewSettings;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        _GlassIconButton(
          onPressed: onBack,
          icon: const Icon(Icons.keyboard_arrow_down_rounded, size: 28),
          tooltip: 'Back',
        ),
        const Spacer(),
        Text(
          'NOW PLAYING',
          style: TextStyle(
            color: Colors.white.withValues(alpha: 0.62),
            fontSize: 11,
            letterSpacing: 1.8,
            fontWeight: FontWeight.w600,
          ),
        ),
        const Spacer(),
        _GlassIconButton(
          onPressed: onOpenQueue,
          icon: const Icon(Icons.queue_music_rounded),
          tooltip: 'Queue',
        ),
        const SizedBox(width: 8),
        _GlassIconButton(
          onPressed: onOpenViewSettings,
          icon: const Icon(Icons.more_vert_rounded),
          tooltip: 'View settings',
        ),
      ],
    );
  }
}

class _SingleBody extends StatelessWidget {
  const _SingleBody({
    super.key,
    required this.showLyrics,
    required this.pureModeActive,
    required this.song,
    required this.player,
    required this.controller,
    required this.fft,
    required this.mode,
    required this.showRingSpectrum,
    required this.ringLayoutActive,
    required this.lyricsAsync,
    required this.lyricLines,
    required this.activeLyricIndex,
    required this.adjustedPositionMs,
    required this.sliderProgress,
    required this.effectiveDuration,
    required this.lyricFontSize,
    required this.lyricAlign,
    required this.lyricSelectable,
    required this.lyricAutoBlur,
    required this.lyricWordByWordAnimation,
    required this.lyricsScroll,
    required this.onToggleLyrics,
    required this.onLyricTap,
    required this.onLyricsUserScroll,
    required this.onProgressChanged,
    required this.onProgressChangeEnd,
    required this.onTogglePlayMode,
    required this.onOpenSongMenu,
  });

  final bool showLyrics;
  final bool pureModeActive;
  final Song song;
  final PlayerControllerState player;
  final PlayerController controller;
  final RustFftSnapshot fft;
  final SpectrumMode mode;
  final bool showRingSpectrum;
  final bool ringLayoutActive;
  final AsyncValue<List<LyricLine>> lyricsAsync;
  final List<LyricLine> lyricLines;
  final int activeLyricIndex;
  final int adjustedPositionMs;
  final double sliderProgress;
  final double effectiveDuration;
  final double lyricFontSize;
  final TextAlign lyricAlign;
  final bool lyricSelectable;
  final bool lyricAutoBlur;
  final bool lyricWordByWordAnimation;
  final ScrollController lyricsScroll;
  final VoidCallback onToggleLyrics;
  final ValueChanged<int> onLyricTap;
  final VoidCallback onLyricsUserScroll;
  final ValueChanged<double> onProgressChanged;
  final ValueChanged<double> onProgressChangeEnd;
  final VoidCallback onTogglePlayMode;
  final VoidCallback onOpenSongMenu;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 280),
            switchInCurve: Curves.easeOutCubic,
            switchOutCurve: Curves.easeInCubic,
            child: showLyrics
                ? _LyricsPanel(
                    key: const ValueKey('lyrics'),
                    lyricsAsync: lyricsAsync,
                    lyricLines: lyricLines,
                    activeLyricIndex: activeLyricIndex,
                    adjustedPositionMs: adjustedPositionMs,
                    lyricFontSize: lyricFontSize,
                    lyricAlign: lyricAlign,
                    lyricSelectable: lyricSelectable,
                    lyricAutoBlur: lyricAutoBlur,
                    lyricWordByWordAnimation: lyricWordByWordAnimation,
                    lyricsScroll: lyricsScroll,
                    onLyricTap: onLyricTap,
                    onUserScroll: onLyricsUserScroll,
                  )
                : Center(
                    key: const ValueKey('cover'),
                    child: LayoutBuilder(
                      builder: (context, constraints) {
                        final maxCover =
                            constraints.maxWidth >= 820 ? 420.0 : 340.0;
                        return ConstrainedBox(
                          constraints: BoxConstraints(
                            maxWidth: maxCover,
                            maxHeight: maxCover,
                          ),
                          child: PlayerStage(
                            song: song,
                            mode: mode,
                            fft: fft,
                            showSpectrum: showRingSpectrum,
                            circularCover: ringLayoutActive,
                            coverFraction: ringLayoutActive ? 0.74 : 0.92,
                          ),
                        );
                      },
                    ),
                  ),
          ),
        ),
        if (!pureModeActive) ...[
          const SizedBox(height: 12),
          _TransportPanel(
            song: song,
            player: player,
            controller: controller,
            sliderProgress: sliderProgress,
            effectiveDuration: effectiveDuration,
            showLyricsToggle: true,
            showLyrics: showLyrics,
            onToggleLyrics: onToggleLyrics,
            onProgressChanged: onProgressChanged,
            onProgressChangeEnd: onProgressChangeEnd,
            onTogglePlayMode: onTogglePlayMode,
            onOpenSongMenu: onOpenSongMenu,
          ),
        ],
      ],
    );
  }
}

class _SplitBody extends StatelessWidget {
  const _SplitBody({
    super.key,
    required this.song,
    required this.player,
    required this.controller,
    required this.fft,
    required this.mode,
    required this.showRingSpectrum,
    required this.ringLayoutActive,
    required this.lyricsAsync,
    required this.lyricLines,
    required this.activeLyricIndex,
    required this.adjustedPositionMs,
    required this.sliderProgress,
    required this.effectiveDuration,
    required this.lyricFontSize,
    required this.lyricAlign,
    required this.lyricSelectable,
    required this.lyricAutoBlur,
    required this.lyricWordByWordAnimation,
    required this.lyricsScroll,
    required this.onLyricTap,
    required this.onLyricsUserScroll,
    required this.onProgressChanged,
    required this.onProgressChangeEnd,
    required this.onTogglePlayMode,
    required this.onOpenSongMenu,
  });

  final Song song;
  final PlayerControllerState player;
  final PlayerController controller;
  final RustFftSnapshot fft;
  final SpectrumMode mode;
  final bool showRingSpectrum;
  final bool ringLayoutActive;
  final AsyncValue<List<LyricLine>> lyricsAsync;
  final List<LyricLine> lyricLines;
  final int activeLyricIndex;
  final int adjustedPositionMs;
  final double sliderProgress;
  final double effectiveDuration;
  final double lyricFontSize;
  final TextAlign lyricAlign;
  final bool lyricSelectable;
  final bool lyricAutoBlur;
  final bool lyricWordByWordAnimation;
  final ScrollController lyricsScroll;
  final ValueChanged<int> onLyricTap;
  final VoidCallback onLyricsUserScroll;
  final ValueChanged<double> onProgressChanged;
  final ValueChanged<double> onProgressChangeEnd;
  final VoidCallback onTogglePlayMode;
  final VoidCallback onOpenSongMenu;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          child: Column(
            children: [
              Expanded(
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(
                      maxWidth: 420,
                      maxHeight: 420,
                    ),
                    child: PlayerStage(
                      song: song,
                      mode: mode,
                      fft: fft,
                      showSpectrum: showRingSpectrum,
                      circularCover: ringLayoutActive,
                      coverFraction: ringLayoutActive ? 0.74 : 0.9,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 12),
              _TransportPanel(
                song: song,
                player: player,
                controller: controller,
                sliderProgress: sliderProgress,
                effectiveDuration: effectiveDuration,
                showLyricsToggle: false,
                showLyrics: false,
                onToggleLyrics: null,
                onProgressChanged: onProgressChanged,
                onProgressChangeEnd: onProgressChangeEnd,
                onTogglePlayMode: onTogglePlayMode,
                onOpenSongMenu: onOpenSongMenu,
              ),
            ],
          ),
        ),
        const SizedBox(width: 16),
        Expanded(
          child: _LyricsPanel(
            lyricsAsync: lyricsAsync,
            lyricLines: lyricLines,
            activeLyricIndex: activeLyricIndex,
            adjustedPositionMs: adjustedPositionMs,
            lyricFontSize: lyricFontSize * 1.12,
            lyricAlign: lyricAlign,
            lyricSelectable: lyricSelectable,
            lyricAutoBlur: lyricAutoBlur,
            lyricWordByWordAnimation: lyricWordByWordAnimation,
            lyricsScroll: lyricsScroll,
            onLyricTap: onLyricTap,
            onUserScroll: onLyricsUserScroll,
          ),
        ),
      ],
    );
  }
}

class _PhoneLandscapeBody extends StatelessWidget {
  const _PhoneLandscapeBody({
    super.key,
    required this.song,
    required this.player,
    required this.controller,
    required this.fft,
    required this.mode,
    required this.showRingSpectrum,
    required this.ringLayoutActive,
    required this.sliderProgress,
    required this.effectiveDuration,
    required this.onProgressChanged,
    required this.onProgressChangeEnd,
    required this.onTogglePlayMode,
    required this.onOpenSongMenu,
    required this.onOpenQueue,
    required this.onBack,
    required this.onOpenViewSettings,
  });

  final Song song;
  final PlayerControllerState player;
  final PlayerController controller;
  final RustFftSnapshot fft;
  final SpectrumMode mode;
  final bool showRingSpectrum;
  final bool ringLayoutActive;
  final double sliderProgress;
  final double effectiveDuration;
  final ValueChanged<double> onProgressChanged;
  final ValueChanged<double> onProgressChangeEnd;
  final VoidCallback onTogglePlayMode;
  final VoidCallback onOpenSongMenu;
  final VoidCallback onOpenQueue;
  final VoidCallback onBack;
  final VoidCallback onOpenViewSettings;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(
          flex: 5,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final byHeight = constraints.maxHeight * 0.72;
              final boundedByMax = byHeight < 360 ? byHeight : 360.0;
              final coverExtent = boundedByMax > 220 ? boundedByMax : 220.0;
              return Center(
                child: ConstrainedBox(
                  constraints: BoxConstraints(
                    maxWidth: coverExtent,
                    maxHeight: coverExtent,
                  ),
                  child: PlayerStage(
                    song: song,
                    mode: mode,
                    fft: fft,
                    showSpectrum: showRingSpectrum,
                    circularCover: ringLayoutActive,
                    coverFraction: ringLayoutActive ? 0.72 : 0.9,
                  ),
                ),
              );
            },
          ),
        ),
        const SizedBox(width: 14),
        Expanded(
          flex: 4,
          child: _TransportPanel(
            song: song,
            player: player,
            controller: controller,
            sliderProgress: sliderProgress,
            effectiveDuration: effectiveDuration,
            showLyricsToggle: false,
            showLyrics: false,
            onToggleLyrics: null,
            onProgressChanged: onProgressChanged,
            onProgressChangeEnd: onProgressChangeEnd,
            onTogglePlayMode: onTogglePlayMode,
            onOpenSongMenu: onOpenSongMenu,
          ),
        ),
        const SizedBox(width: 10),
        Column(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            _GlassIconButton(
              onPressed: onBack,
              icon: const Icon(Icons.keyboard_arrow_down_rounded, size: 26),
              tooltip: 'Back',
            ),
            _GlassIconButton(
              onPressed: onOpenViewSettings,
              icon: const Icon(Icons.more_vert_rounded),
              tooltip: 'View settings',
            ),
            _GlassIconButton(
              onPressed: onOpenQueue,
              icon: const Icon(Icons.queue_music_rounded),
              tooltip: 'Queue',
            ),
          ],
        ),
      ],
    );
  }
}

class _TransportPanel extends StatelessWidget {
  const _TransportPanel({
    required this.song,
    required this.player,
    required this.controller,
    required this.sliderProgress,
    required this.effectiveDuration,
    required this.showLyricsToggle,
    required this.showLyrics,
    required this.onToggleLyrics,
    required this.onProgressChanged,
    required this.onProgressChangeEnd,
    required this.onTogglePlayMode,
    required this.onOpenSongMenu,
  });

  final Song song;
  final PlayerControllerState player;
  final PlayerController controller;
  final double sliderProgress;
  final double effectiveDuration;
  final bool showLyricsToggle;
  final bool showLyrics;
  final VoidCallback? onToggleLyrics;
  final ValueChanged<double> onProgressChanged;
  final ValueChanged<double> onProgressChangeEnd;
  final VoidCallback onTogglePlayMode;
  final VoidCallback onOpenSongMenu;

  @override
  Widget build(BuildContext context) {
    final audioInfo = _formatAudioInfo(song);
    final max = effectiveDuration > 0 ? effectiveDuration : 1.0;
    final clampedProgress = sliderProgress.clamp(0.0, max);

    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 0, 4, 4),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      song.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 22,
                        fontWeight: FontWeight.w700,
                        letterSpacing: -0.2,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      song.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.7),
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ],
                ),
              ),
              if (showLyricsToggle)
                IconButton(
                  onPressed: onToggleLyrics,
                  tooltip: showLyrics ? 'Show cover' : 'Show lyrics',
                  icon: Icon(
                    showLyrics ? Icons.album_rounded : Icons.lyrics_rounded,
                    color: Colors.white.withValues(alpha: 0.9),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 10),
          SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 3.2,
              activeTrackColor: Colors.white,
              inactiveTrackColor: Colors.white.withValues(alpha: 0.24),
              thumbColor: Colors.white,
              overlayColor: Colors.white.withValues(alpha: 0.2),
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
            ),
            child: Slider(
              min: 0,
              max: max,
              value: clampedProgress,
              onChanged: onProgressChanged,
              onChangeEnd: onProgressChangeEnd,
            ),
          ),
          Row(
            children: [
              Text(
                _formatDuration(clampedProgress),
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.68),
                  fontSize: 12,
                  fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
                ),
              ),
              const Spacer(),
              if (audioInfo != null)
                Text(
                  audioInfo,
                  style: TextStyle(
                    color: Colors.white.withValues(alpha: 0.48),
                    fontSize: 11,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              if (audioInfo != null) const Spacer(),
              Text(
                _formatDuration(effectiveDuration),
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.68),
                  fontSize: 12,
                  fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              IconButton(
                onPressed: onTogglePlayMode,
                tooltip: 'Play mode',
                icon: Icon(
                  _playModeIcon(player.mode),
                  color: Colors.white.withValues(alpha: 0.72),
                ),
              ),
              const Spacer(),
              IconButton(
                onPressed: controller.previous,
                icon: Icon(
                  PhosphorIcons.skipBack(),
                  color: Colors.white,
                  size: 28,
                ),
                tooltip: 'Previous',
              ),
              const SizedBox(width: 6),
              IconButton.filled(
                onPressed: controller.togglePlayPause,
                style: IconButton.styleFrom(
                  backgroundColor: Colors.white,
                  foregroundColor: Colors.black,
                  minimumSize: const Size(58, 58),
                ),
                icon: Icon(
                  player.isPlaying
                      ? PhosphorIcons.pause(PhosphorIconsStyle.fill)
                      : PhosphorIcons.play(PhosphorIconsStyle.fill),
                ),
                tooltip: player.isPlaying ? 'Pause' : 'Play',
              ),
              const SizedBox(width: 6),
              IconButton(
                onPressed: controller.next,
                icon: Icon(
                  PhosphorIcons.skipForward(),
                  color: Colors.white,
                  size: 28,
                ),
                tooltip: 'Next',
              ),
              const Spacer(),
              IconButton(
                onPressed: onOpenSongMenu,
                tooltip: 'More',
                icon: Icon(
                  Icons.more_vert_rounded,
                  color: Colors.white.withValues(alpha: 0.72),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _LyricsPanel extends StatelessWidget {
  const _LyricsPanel({
    super.key,
    required this.lyricsAsync,
    required this.lyricLines,
    required this.activeLyricIndex,
    required this.adjustedPositionMs,
    required this.lyricFontSize,
    required this.lyricAlign,
    required this.lyricSelectable,
    required this.lyricAutoBlur,
    required this.lyricWordByWordAnimation,
    required this.lyricsScroll,
    required this.onLyricTap,
    required this.onUserScroll,
  });

  final AsyncValue<List<LyricLine>> lyricsAsync;
  final List<LyricLine> lyricLines;
  final int activeLyricIndex;
  final int adjustedPositionMs;
  final double lyricFontSize;
  final TextAlign lyricAlign;
  final bool lyricSelectable;
  final bool lyricAutoBlur;
  final bool lyricWordByWordAnimation;
  final ScrollController lyricsScroll;
  final ValueChanged<int> onLyricTap;
  final VoidCallback onUserScroll;

  @override
  Widget build(BuildContext context) {
    if (lyricsAsync.isLoading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (lyricLines.isEmpty) {
      return Center(
        child: Text(
          'No lyrics found for current song.',
          style: TextStyle(color: Colors.white.withValues(alpha: 0.68)),
        ),
      );
    }

    final introWaiting =
        activeLyricIndex < 0 && adjustedPositionMs < lyricLines.first.startMs;
    final itemCount = lyricLines.length + (introWaiting ? 1 : 0);

    return NotificationListener<UserScrollNotification>(
      onNotification: (notification) {
        if (notification.direction != ScrollDirection.idle) {
          onUserScroll();
        }
        return false;
      },
      child: ListView.builder(
        controller: lyricsScroll,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 90),
        itemCount: itemCount,
        itemExtent: 78,
        itemBuilder: (context, index) {
          if (introWaiting && index == 0) {
            return const Center(child: _LyricWaitingDots());
          }

          final lineIndex = introWaiting ? index - 1 : index;
          final line = lyricLines[lineIndex];
          final active = lineIndex == activeLyricIndex;
          final blur = lyricAutoBlur
              ? _lyricBlurForIndex(lineIndex, activeLyricIndex)
              : 0.0;

          final lineStyle = TextStyle(
            fontSize: active ? lyricFontSize + 2 : lyricFontSize,
            fontWeight: active ? FontWeight.w700 : FontWeight.w500,
            color: active
                ? Colors.white
                : Colors.white.withValues(alpha: 0.45),
            height: 1.28,
          );

          return AnimatedOpacity(
            duration: const Duration(milliseconds: 180),
            opacity: active ? 1 : 0.92,
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 180),
              curve: Curves.easeOut,
              alignment: Alignment.center,
              transformAlignment: _alignToAlignment(lyricAlign),
              transform: Matrix4.identity()
                ..scale(active ? 1.0 : 0.98, active ? 1.0 : 0.98),
              child: InkWell(
                borderRadius: BorderRadius.circular(10),
                onTap: lyricSelectable ? null : () => onLyricTap(line.startMs),
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 6),
                  child: Center(
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 220),
                      curve: Curves.easeOut,
                      transform: Matrix4.identity(),
                      foregroundDecoration: blur <= 0
                          ? null
                          : BoxDecoration(
                              color: Colors.transparent,
                              boxShadow: [
                                BoxShadow(
                                  color: Colors.black.withValues(alpha: 0.0001),
                                  blurRadius: blur,
                                  spreadRadius: blur * 0.2,
                                ),
                              ],
                            ),
                      child: _buildLyricText(
                        line: line,
                        active: active,
                        positionMs: adjustedPositionMs,
                        lineStyle: lineStyle,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  Widget _buildLyricText({
    required LyricLine line,
    required bool active,
    required int positionMs,
    required TextStyle lineStyle,
  }) {
    if (!lyricWordByWordAnimation || line.tokens.isEmpty || lyricSelectable) {
      if (lyricSelectable) {
        return SelectableText(
          line.text,
          textAlign: lyricAlign,
          style: lineStyle,
        );
      }
      return Text(
        line.text,
        textAlign: lyricAlign,
        style: lineStyle,
      );
    }

    return SizedBox(
      width: double.infinity,
      child: KaraokeLine(
        line: line,
        positionMs: positionMs,
        active: active,
      ),
    );
  }
}

class _LyricWaitingDots extends StatelessWidget {
  const _LyricWaitingDots();

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        for (var i = 0; i < 3; i++)
          _PulseDot(delayMs: i * 260),
      ],
    );
  }
}

class _PulseDot extends StatefulWidget {
  const _PulseDot({required this.delayMs});

  final int delayMs;

  @override
  State<_PulseDot> createState() => _PulseDotState();
}

class _PulseDotState extends State<_PulseDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, _) {
          final shifted =
              ((_controller.value + widget.delayMs / 1200.0) % 1.0).toDouble();
          final opacity = 0.35 + (1 - (shifted - 0.5).abs() * 2) * 0.6;
          return Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: opacity.clamp(0.2, 1.0)),
              shape: BoxShape.circle,
            ),
          );
        },
      ),
    );
  }
}

class _QueueSheet extends StatelessWidget {
  const _QueueSheet({
    required this.player,
    required this.onJumpTo,
    required this.onRemove,
    required this.onClear,
  });

  final PlayerControllerState player;
  final ValueChanged<int> onJumpTo;
  final ValueChanged<int> onRemove;
  final Future<void> Function() onClear;

  @override
  Widget build(BuildContext context) {
    final queue = player.queue;
    return SafeArea(
      top: false,
      child: Container(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * 0.8,
        ),
        decoration: BoxDecoration(
          color: const Color(0xFF121214).withValues(alpha: 0.92),
          borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
          border: Border.all(color: Colors.white.withValues(alpha: 0.12)),
        ),
        child: Column(
          children: [
            const SizedBox(height: 10),
            Container(
              width: 44,
              height: 4,
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.3),
                borderRadius: BorderRadius.circular(999),
              ),
            ),
            const SizedBox(height: 12),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Row(
                children: [
                  const Text(
                    'Playing Queue',
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 17,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const Spacer(),
                  TextButton(
                    onPressed: () => unawaited(onClear()),
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
                        'Queue is empty.',
                        style: TextStyle(
                          color: Colors.white.withValues(alpha: 0.68),
                        ),
                      ),
                    )
                  : ListView.builder(
                      itemCount: queue.length,
                      itemBuilder: (context, index) {
                        final song = queue[index];
                        final active = player.currentIndex == index;
                        return ListTile(
                          onTap: () => onJumpTo(index),
                          leading: ClipRRect(
                            borderRadius: BorderRadius.circular(8),
                            child: CoverArt(
                              width: 44,
                              height: 44,
                              coverHash: song.coverHash,
                              streamInfo: song.streamInfo,
                              size: CoverArtSize.small,
                            ),
                          ),
                          title: Text(
                            song.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: active
                                  ? Colors.white
                                  : Colors.white.withValues(alpha: 0.86),
                              fontWeight:
                                  active ? FontWeight.w700 : FontWeight.w500,
                            ),
                          ),
                          subtitle: Text(
                            song.artist,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: Colors.white.withValues(alpha: 0.6),
                            ),
                          ),
                          trailing: IconButton(
                            onPressed: () => onRemove(index),
                            icon: Icon(
                              Icons.close_rounded,
                              color: Colors.white.withValues(alpha: 0.5),
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
  }
}

class _ViewSettingsSheet extends StatelessWidget {
  const _ViewSettingsSheet({
    required this.showDesktopSplitToggle,
    required this.splitViewEnabled,
    required this.pureModeFeatureEnabled,
    required this.pureModeActive,
    required this.proSpectrumActive,
    required this.mode,
    required this.onSplitViewChanged,
    required this.onPureModeChanged,
    required this.onSpectrumModeChanged,
  });

  final bool showDesktopSplitToggle;
  final bool splitViewEnabled;
  final bool pureModeFeatureEnabled;
  final bool pureModeActive;
  final bool proSpectrumActive;
  final SpectrumMode mode;
  final ValueChanged<bool> onSplitViewChanged;
  final ValueChanged<bool> onPureModeChanged;
  final ValueChanged<SpectrumMode> onSpectrumModeChanged;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      top: false,
      child: Container(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * 0.72,
        ),
        decoration: BoxDecoration(
          color: const Color(0xFF151518).withValues(alpha: 0.96),
          borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
          border: Border.all(color: Colors.white.withValues(alpha: 0.12)),
        ),
        child: ListView(
          padding: const EdgeInsets.fromLTRB(16, 14, 16, 18),
          children: [
            Center(
              child: Container(
                width: 44,
                height: 4,
                decoration: BoxDecoration(
                  color: Colors.white.withValues(alpha: 0.3),
                  borderRadius: BorderRadius.circular(999),
                ),
              ),
            ),
            const SizedBox(height: 16),
            Text(
              'View Settings',
              style: TextStyle(
                color: Colors.white.withValues(alpha: 0.85),
                fontSize: 13,
                fontWeight: FontWeight.w700,
                letterSpacing: 0.4,
              ),
            ),
            const SizedBox(height: 10),
            if (showDesktopSplitToggle)
              SwitchListTile.adaptive(
                contentPadding: EdgeInsets.zero,
                title: const Text(
                  'Split View',
                  style: TextStyle(color: Colors.white),
                ),
                subtitle: Text(
                  'Desktop two-column layout',
                  style: TextStyle(color: Colors.white.withValues(alpha: 0.58)),
                ),
                value: splitViewEnabled,
                onChanged: onSplitViewChanged,
              ),
            if (pureModeFeatureEnabled)
              SwitchListTile.adaptive(
                contentPadding: EdgeInsets.zero,
                title: const Text(
                  'Pure Mode',
                  style: TextStyle(color: Colors.white),
                ),
                subtitle: Text(
                  'Hide lyrics and controls for minimalist stage',
                  style: TextStyle(color: Colors.white.withValues(alpha: 0.58)),
                ),
                value: pureModeActive,
                onChanged: onPureModeChanged,
              ),
            if (proSpectrumActive) ...[
              const SizedBox(height: 10),
              Text(
                'Spectrum Mode',
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.85),
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  letterSpacing: 0.4,
                ),
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final item in SpectrumMode.values)
                    ChoiceChip(
                      label: Text(_spectrumLabel(item)),
                      selected: mode == item,
                      onSelected: (_) => onSpectrumModeChanged(item),
                    ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _GlassIconButton extends StatelessWidget {
  const _GlassIconButton({
    required this.onPressed,
    required this.icon,
    required this.tooltip,
  });

  final VoidCallback onPressed;
  final Widget icon;
  final String tooltip;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      onPressed: onPressed,
      tooltip: tooltip,
      style: IconButton.styleFrom(
        backgroundColor: Colors.black.withValues(alpha: 0.2),
        foregroundColor: Colors.white,
        minimumSize: const Size(42, 42),
      ),
      icon: icon,
    );
  }
}

IconData _playModeIcon(PlayMode mode) {
  switch (mode) {
    case PlayMode.sequence:
      return Icons.repeat_rounded;
    case PlayMode.shuffle:
      return Icons.shuffle_rounded;
    case PlayMode.repeatOne:
      return Icons.repeat_one_rounded;
  }
}

TextAlign _lyricsAlign(String raw) {
  switch (raw) {
    case 'center':
      return TextAlign.center;
    case 'right':
      return TextAlign.right;
    case 'left':
    default:
      return TextAlign.left;
  }
}

Alignment _alignToAlignment(TextAlign align) {
  switch (align) {
    case TextAlign.right:
    case TextAlign.end:
      return Alignment.centerRight;
    case TextAlign.center:
      return Alignment.center;
    case TextAlign.left:
    case TextAlign.start:
    default:
      return Alignment.centerLeft;
  }
}

double _lyricBlurForIndex(int index, int currentIndex) {
  if (currentIndex < 0) return 0;
  final distance = (index - currentIndex).abs();
  if (distance == 0) return 0;
  return (distance * 0.45).clamp(0.0, 8.0);
}

String _formatDuration(double seconds) {
  if (seconds.isNaN || seconds < 0) {
    return '--:--';
  }
  final total = seconds.floor();
  final hours = total ~/ 3600;
  final minutes = (total % 3600) ~/ 60;
  final secs = total % 60;
  if (hours > 0) {
    return '$hours:${minutes.toString().padLeft(2, '0')}:${secs.toString().padLeft(2, '0')}';
  }
  return '${minutes.toString().padLeft(2, '0')}:${secs.toString().padLeft(2, '0')}';
}

String? _formatAudioInfo(Song song) {
  final parts = <String>[];
  if (song.format != null && song.format!.trim().isNotEmpty) {
    parts.add(song.format!.trim().toUpperCase());
  }
  if (song.bitDepth != null) {
    parts.add('${song.bitDepth}bit');
  }
  if (song.sampleRate != null) {
    final value = song.sampleRate!;
    if (value >= 1000) {
      final khz = value / 1000.0;
      final rendered = khz % 1 == 0 ? khz.toStringAsFixed(0) : khz.toStringAsFixed(1);
      parts.add('${rendered}kHz');
    } else {
      parts.add('${value}Hz');
    }
  } else if (song.bitrate != null) {
    parts.add('${song.bitrate}kbps');
  }
  if (parts.isEmpty) {
    return null;
  }
  return parts.join(' ');
}

String _spectrumLabel(SpectrumMode mode) {
  switch (mode) {
    case SpectrumMode.wave:
      return 'Wave';
    case SpectrumMode.godRing:
      return 'God Ring';
    case SpectrumMode.diffusionRing:
      return 'Diffusion';
    case SpectrumMode.trippyRipple:
      return 'Trippy Ripple';
    case SpectrumMode.attachmentRing:
      return 'Attachment Ring';
    case SpectrumMode.rotatingCover:
      return 'Rotating Cover';
    case SpectrumMode.bessel:
      return 'Bessel';
    case SpectrumMode.columnar:
      return 'Columnar';
  }
}
