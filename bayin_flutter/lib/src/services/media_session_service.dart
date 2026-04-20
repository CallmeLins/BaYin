import 'package:audio_service/audio_service.dart';

import '../models/models.dart';

class MediaSessionCallbacks {
  const MediaSessionCallbacks({
    required this.onPlay,
    required this.onPause,
    required this.onStop,
    required this.onSeek,
    required this.onSkipToNext,
    required this.onSkipToPrevious,
  });

  final Future<void> Function() onPlay;
  final Future<void> Function() onPause;
  final Future<void> Function() onStop;
  final Future<void> Function(Duration position) onSeek;
  final Future<void> Function() onSkipToNext;
  final Future<void> Function() onSkipToPrevious;
}

class MediaSessionService {
  MediaSessionService._();

  static final MediaSessionService instance = MediaSessionService._();

  AudioHandler? _handler;
  _BayinAudioHandler? _bridge;

  Future<void> init() async {
    if (_handler != null) {
      return;
    }

    _handler = await AudioService.init(
      builder: () {
        final handler = _BayinAudioHandler();
        _bridge = handler;
        return handler;
      },
      config: const AudioServiceConfig(
        androidNotificationChannelId: 'com.bayin.playback',
        androidNotificationChannelName: 'BaYin Playback',
        androidNotificationOngoing: true,
      ),
    );
  }

  void bindCallbacks(MediaSessionCallbacks callbacks) {
    _bridge?.callbacks = callbacks;
  }

  void syncPlayerState(PlayerControllerState state) {
    _bridge?.syncFromPlayerState(state);
  }
}

class _BayinAudioHandler extends BaseAudioHandler
    with QueueHandler, SeekHandler {
  MediaSessionCallbacks? callbacks;

  _BayinAudioHandler() {
    playbackState.add(
      PlaybackState(
        processingState: AudioProcessingState.idle,
        playing: false,
      ),
    );
  }

  void syncFromPlayerState(PlayerControllerState state) {
    final queueItems = state.queue.map(_songToMediaItem).toList(growable: false);
    queue.add(queueItems);

    final index = state.currentIndex;
    if (index != null && index >= 0 && index < queueItems.length) {
      mediaItem.add(queueItems[index]);
    } else {
      mediaItem.add(null);
    }

    playbackState.add(
      playbackState.value.copyWith(
        controls: const <MediaControl>[
          MediaControl.skipToPrevious,
          MediaControl.play,
          MediaControl.pause,
          MediaControl.stop,
          MediaControl.skipToNext,
        ],
        systemActions: const <MediaAction>{
          MediaAction.seek,
          MediaAction.skipToNext,
          MediaAction.skipToPrevious,
        },
        androidCompactActionIndices: const <int>[0, 2, 4],
        processingState: _processingStateOf(state),
        playing: state.isPlaying,
        updatePosition: Duration(milliseconds: (state.positionSecs * 1000).round()),
        bufferedPosition: Duration(milliseconds: (state.positionSecs * 1000).round()),
        speed: 1,
        queueIndex: index,
      ),
    );
  }

  @override
  Future<void> play() async {
    await callbacks?.onPlay();
  }

  @override
  Future<void> pause() async {
    await callbacks?.onPause();
  }

  @override
  Future<void> stop() async {
    await callbacks?.onStop();
  }

  @override
  Future<void> seek(Duration position) async {
    await callbacks?.onSeek(position);
  }

  @override
  Future<void> skipToNext() async {
    await callbacks?.onSkipToNext();
  }

  @override
  Future<void> skipToPrevious() async {
    await callbacks?.onSkipToPrevious();
  }

  static MediaItem _songToMediaItem(Song song) {
    return MediaItem(
      id: song.id,
      title: song.title,
      artist: song.artist,
      album: song.album,
      duration: Duration(milliseconds: (song.duration * 1000).round()),
      extras: <String, dynamic>{
        'filePath': song.filePath,
      },
    );
  }

  static AudioProcessingState _processingStateOf(PlayerControllerState state) {
    if (state.isBusy) {
      return AudioProcessingState.buffering;
    }
    if (state.hasSong) {
      return AudioProcessingState.ready;
    }
    return AudioProcessingState.idle;
  }
}
