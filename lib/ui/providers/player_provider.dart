import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../rust/rust_api.dart';
import '../services/media_session_service.dart';

final playerControllerProvider =
    NotifierProvider<PlayerController, PlayerControllerState>(
      PlayerController.new,
    );

class PlayerController extends Notifier<PlayerControllerState> {
  Timer? _pollTimer;
  bool _handlingEnded = false;
  bool _windowsTaskbarInitialized = false;
  bool _isHandlingWindowsTaskbarEvents = false;
  bool? _lastTaskbarIsPlaying;
  bool? _lastTaskbarCanPrevious;
  bool? _lastTaskbarCanNext;
  String? _lastTaskbarTooltip;
  final Random _random = Random();

  @override
  PlayerControllerState build() {
    final initial = PlayerControllerState.initial();
    MediaSessionService.instance.bindCallbacks(
      MediaSessionCallbacks(
        onPlay: resume,
        onPause: pause,
        onStop: stop,
        onSeek: (position) async {
          await seek(position.inMilliseconds / 1000);
        },
        onSkipToNext: next,
        onSkipToPrevious: previous,
      ),
    );
    MediaSessionService.instance.syncPlayerState(initial);
    _tryInitWindowsTaskbar();
    _pollTimer ??= Timer.periodic(
      const Duration(milliseconds: 400),
      (_) => unawaited(_refresh()),
    );
    ref.onDispose(() {
      _pollTimer?.cancel();
      _pollTimer = null;
    });
    Future<void>.microtask(_refresh);
    return initial;
  }

  Future<void> playQueue(List<Song> queue, {required int startIndex}) async {
    if (queue.isEmpty || startIndex < 0 || startIndex >= queue.length) {
      return;
    }

    final song = queue[startIndex];
    final source = _resolveSource(song);
    _setState(state.copyWith(
      queue: List<Song>.unmodifiable(queue),
      currentIndex: startIndex,
      durationSecs: song.duration,
      positionSecs: 0,
      isBusy: true,
      error: null,
    ));

    try {
      RustApi.instance.audioPlay(source);
      await _refresh();
    } catch (error) {
      _setState(state.copyWith(isBusy: false, error: '$error'));
    }
  }

  Future<void> playSong(Song song) {
    return playQueue(<Song>[song], startIndex: 0);
  }

  Future<void> jumpTo(int index) async {
    if (index < 0 || index >= state.queue.length) {
      return;
    }
    await playQueue(state.queue, startIndex: index);
  }

  Future<void> togglePlayPause() async {
    try {
      if (state.isPlaying) {
        RustApi.instance.audioPause();
      } else if (state.currentSong != null) {
        RustApi.instance.audioResume();
      } else if (state.queue.isNotEmpty) {
        await playQueue(state.queue, startIndex: state.currentIndex ?? 0);
        return;
      } else {
        return;
      }
      await _refresh();
    } catch (error) {
      _setState(state.copyWith(error: '$error'));
    }
  }

  Future<void> pause() async {
    try {
      RustApi.instance.audioPause();
      await _refresh();
    } catch (error) {
      _setState(state.copyWith(error: '$error'));
    }
  }

  Future<void> resume() async {
    try {
      RustApi.instance.audioResume();
      await _refresh();
    } catch (error) {
      _setState(state.copyWith(error: '$error'));
    }
  }

  Future<void> stop() async {
    try {
      RustApi.instance.audioStop();
      _setState(state.copyWith(
        isPlaying: false,
        positionSecs: 0,
        durationSecs: state.currentSong?.duration ?? 0,
        error: null,
      ));
      await _refresh();
    } catch (error) {
      _setState(state.copyWith(error: '$error'));
    }
  }

  Future<void> seek(double positionSecs) async {
    try {
      RustApi.instance.audioSeek(positionSecs);
      _setState(state.copyWith(
        positionSecs: positionSecs.clamp(0.0, state.durationSecs),
      ));
      await _refresh();
    } catch (error) {
      _setState(state.copyWith(error: '$error'));
    }
  }

  Future<void> setVolume(double volume) async {
    final clamped = volume.clamp(0.0, 1.0).toDouble();
    _setState(state.copyWith(volume: clamped));
    try {
      RustApi.instance.audioSetVolume(clamped);
    } catch (error) {
      _setState(state.copyWith(error: '$error'));
    }
  }

  Future<void> next() async {
    final nextIndex = _nextIndexForAdvance(manual: true);
    if (nextIndex == null) {
      return;
    }
    await jumpTo(nextIndex);
  }

  Future<void> previous() async {
    if (state.positionSecs > 3) {
      await seek(0);
      return;
    }

    final index = state.currentIndex;
    if (index == null) {
      return;
    }
    final previousIndex = index > 0 ? index - 1 : 0;
    await jumpTo(previousIndex);
  }

  void setPlayMode(PlayMode mode) {
    _setState(state.copyWith(mode: mode));
  }

  void enqueueNext(Song song) {
    if (!state.hasSong && state.queue.isEmpty) {
      unawaited(playSong(song));
      return;
    }

    final queue = state.queue.toList();
    final currentIndex = state.currentIndex;
    if (currentIndex == null) {
      queue.add(song);
    } else {
      final insertIndex = min(currentIndex + 1, queue.length);
      queue.insert(insertIndex, song);
    }
    _setState(state.copyWith(queue: List<Song>.unmodifiable(queue), error: null));
  }

  Future<void> removeFromQueue(int index) async {
    final currentQueue = state.queue;
    if (index < 0 || index >= currentQueue.length) {
      return;
    }

    final nextQueue = currentQueue.toList()..removeAt(index);
    final currentIndex = state.currentIndex;
    if (nextQueue.isEmpty) {
      try {
        RustApi.instance.audioStop();
      } catch (_) {
        // Keep local queue clearing resilient if engine is unavailable.
      }
      _setState(
        state.copyWith(
          queue: const <Song>[],
          currentIndex: null,
          isPlaying: false,
          positionSecs: 0,
          durationSecs: 0,
          error: null,
        ),
      );
      return;
    }

    if (currentIndex == null) {
      _setState(
        state.copyWith(
          queue: List<Song>.unmodifiable(nextQueue),
          error: null,
        ),
      );
      return;
    }

    if (index == currentIndex) {
      final fallbackIndex = index.clamp(0, nextQueue.length - 1).toInt();
      await playQueue(nextQueue, startIndex: fallbackIndex);
      return;
    }

    final adjustedIndex = index < currentIndex ? currentIndex - 1 : currentIndex;
    _setState(
      state.copyWith(
        queue: List<Song>.unmodifiable(nextQueue),
        currentIndex: adjustedIndex,
        error: null,
      ),
    );
  }

  Future<void> clearQueue() async {
    try {
      RustApi.instance.audioStop();
    } catch (_) {
      // Keep local queue clearing resilient if engine is unavailable.
    }
    _setState(
      state.copyWith(
        queue: const <Song>[],
        currentIndex: null,
        isPlaying: false,
        positionSecs: 0,
        durationSecs: 0,
        error: null,
      ),
    );
  }

  Future<void> _refresh() async {
    try {
      final rustState = RustApi.instance.getPlaybackState();
      final nextState = state.copyWith(
        isPlaying: rustState.isPlaying,
        positionSecs: rustState.positionSecs,
        durationSecs: rustState.durationSecs > 0
            ? rustState.durationSecs
            : (state.currentSong?.duration ?? state.durationSecs),
        volume: rustState.volume.clamp(0.0, 1.0).toDouble(),
        isBusy: false,
        error: null,
      );
      _setState(nextState);
      await _pollWindowsTaskbarEvents();

      if (rustState.hasEnded && !_handlingEnded && state.currentIndex != null) {
        _handlingEnded = true;
        try {
          final nextIndex = _nextIndexForAdvance(manual: false);
          if (nextIndex != null) {
            await jumpTo(nextIndex);
          } else {
            _setState(state.copyWith(
              isPlaying: false,
              positionSecs: state.durationSecs,
            ));
          }
        } finally {
          _handlingEnded = false;
        }
      }
    } catch (error) {
      _setState(state.copyWith(isBusy: false, error: '$error'));
    }
  }

  void _setState(PlayerControllerState nextState) {
    state = nextState;
    MediaSessionService.instance.syncPlayerState(nextState);
    _syncWindowsTaskbar(nextState);
  }

  int? _nextIndexForAdvance({required bool manual}) {
    final currentIndex = state.currentIndex;
    final queueLength = state.queue.length;
    if (currentIndex == null || queueLength == 0) {
      return null;
    }

    if (!manual && state.mode == PlayMode.repeatOne) {
      return currentIndex;
    }

    if (state.mode == PlayMode.shuffle && queueLength > 1) {
      var nextIndex = currentIndex;
      while (nextIndex == currentIndex) {
        nextIndex = _random.nextInt(queueLength);
      }
      return nextIndex;
    }

    final nextIndex = currentIndex + 1;
    if (nextIndex < queueLength) {
      return nextIndex;
    }
    return null;
  }

  String _resolveSource(Song song) {
    final direct = _extractPlayableSource(song.filePath);
    if (direct != null) {
      return direct;
    }

    final stream = _extractPlayableSource(song.streamInfo);
    if (stream != null) {
      return stream;
    }

    throw StateError('Song does not have a playable source: ${song.title}');
  }

  String? _extractPlayableSource(String? raw) {
    if (raw == null) {
      return null;
    }

    final trimmed = raw.trim();
    if (trimmed.isEmpty) {
      return null;
    }
    if (!trimmed.startsWith('{')) {
      return trimmed;
    }

    try {
      final decoded = jsonDecode(trimmed);
      return _findSourceInJson(decoded);
    } catch (_) {
      return null;
    }
  }

  String? _findSourceInJson(dynamic value) {
    if (value is String) {
      final trimmed = value.trim();
      if (trimmed.isNotEmpty &&
          !trimmed.startsWith('{') &&
          (trimmed.startsWith('http://') ||
              trimmed.startsWith('https://') ||
              trimmed.contains(':\\') ||
              trimmed.startsWith('/'))) {
        return trimmed;
      }
      return null;
    }

    if (value is List<dynamic>) {
      for (final item in value) {
        final result = _findSourceInJson(item);
        if (result != null) {
          return result;
        }
      }
      return null;
    }

    if (value is Map) {
      for (final key in <String>[
        'url',
        'streamUrl',
        'source',
        'path',
        'filePath',
      ]) {
        final candidate = _findSourceInJson(value[key]);
        if (candidate != null) {
          return candidate;
        }
      }
      for (final entry in value.values) {
        final result = _findSourceInJson(entry);
        if (result != null) {
          return result;
        }
      }
    }

    return null;
  }

  bool get _isWindowsDesktop => !kIsWeb && Platform.isWindows;

  void _tryInitWindowsTaskbar() {
    if (!_isWindowsDesktop || _windowsTaskbarInitialized) {
      return;
    }
    try {
      RustApi.instance.windowsTaskbarInit();
      _windowsTaskbarInitialized = true;
    } catch (_) {
      _windowsTaskbarInitialized = false;
    }
  }

  void _syncWindowsTaskbar(PlayerControllerState snapshot) {
    if (!_isWindowsDesktop) {
      return;
    }
    if (!_windowsTaskbarInitialized) {
      _tryInitWindowsTaskbar();
      if (!_windowsTaskbarInitialized) {
        return;
      }
    }

    final canPrevious = _canGoPrevious(snapshot);
    final canNext = _canGoNext(snapshot);
    final tooltip = _taskbarTooltip(snapshot);

    if (_lastTaskbarIsPlaying == snapshot.isPlaying &&
        _lastTaskbarCanPrevious == canPrevious &&
        _lastTaskbarCanNext == canNext &&
        _lastTaskbarTooltip == tooltip) {
      return;
    }

    try {
      RustApi.instance.windowsTaskbarUpdate(
        RustWindowsTaskbarState(
          isPlaying: snapshot.isPlaying,
          canPrevious: canPrevious,
          canNext: canNext,
          tooltip: tooltip,
        ),
      );
      _lastTaskbarIsPlaying = snapshot.isPlaying;
      _lastTaskbarCanPrevious = canPrevious;
      _lastTaskbarCanNext = canNext;
      _lastTaskbarTooltip = tooltip;
    } catch (_) {
      _windowsTaskbarInitialized = false;
    }
  }

  Future<void> _pollWindowsTaskbarEvents() async {
    if (!_isWindowsDesktop || _isHandlingWindowsTaskbarEvents) {
      return;
    }
    if (!_windowsTaskbarInitialized) {
      _tryInitWindowsTaskbar();
      if (!_windowsTaskbarInitialized) {
        return;
      }
    }

    _isHandlingWindowsTaskbarEvents = true;
    try {
      final events = RustApi.instance.pollWindowsTaskbarEvents();
      for (final event in events) {
        switch (event.action) {
          case 'previous':
            await previous();
            break;
          case 'next':
            await next();
            break;
          case 'playPause':
            await togglePlayPause();
            break;
          default:
            break;
        }
      }
    } catch (_) {
      _windowsTaskbarInitialized = false;
    } finally {
      _isHandlingWindowsTaskbarEvents = false;
    }
  }

  bool _canGoPrevious(PlayerControllerState snapshot) {
    final index = snapshot.currentIndex;
    if (index == null) {
      return false;
    }
    return snapshot.positionSecs > 3 || index > 0;
  }

  bool _canGoNext(PlayerControllerState snapshot) {
    final index = snapshot.currentIndex;
    if (index == null) {
      return false;
    }
    if (snapshot.mode == PlayMode.shuffle) {
      return snapshot.queue.length > 1;
    }
    return index + 1 < snapshot.queue.length;
  }

  String? _taskbarTooltip(PlayerControllerState snapshot) {
    final song = snapshot.currentSong;
    if (song == null) {
      return null;
    }
    final title = song.title.trim();
    final artist = song.artist.trim();
    if (title.isEmpty && artist.isEmpty) {
      return null;
    }
    if (artist.isEmpty) {
      return title;
    }
    if (title.isEmpty) {
      return artist;
    }
    return '$title - $artist';
  }
}
