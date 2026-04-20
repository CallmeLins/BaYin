import 'song.dart';
import 'play_mode.dart';

class PlayerControllerState {
  const PlayerControllerState({
    required this.queue,
    required this.currentIndex,
    required this.isPlaying,
    required this.positionSecs,
    required this.durationSecs,
    required this.volume,
    required this.mode,
    required this.isBusy,
    this.error,
  });

  factory PlayerControllerState.initial() {
    return const PlayerControllerState(
      queue: <Song>[],
      currentIndex: null,
      isPlaying: false,
      positionSecs: 0,
      durationSecs: 0,
      volume: 1,
      mode: PlayMode.sequence,
      isBusy: false,
      error: null,
    );
  }

  static const Object _sentinel = Object();

  final List<Song> queue;
  final int? currentIndex;
  final bool isPlaying;
  final double positionSecs;
  final double durationSecs;
  final double volume;
  final PlayMode mode;
  final bool isBusy;
  final String? error;

  Song? get currentSong {
    final index = currentIndex;
    if (index == null || index < 0 || index >= queue.length) {
      return null;
    }
    return queue[index];
  }

  bool get hasSong => currentSong != null;

  PlayerControllerState copyWith({
    List<Song>? queue,
    Object? currentIndex = _sentinel,
    bool? isPlaying,
    double? positionSecs,
    double? durationSecs,
    double? volume,
    PlayMode? mode,
    bool? isBusy,
    Object? error = _sentinel,
  }) {
    return PlayerControllerState(
      queue: queue ?? this.queue,
      currentIndex:
          currentIndex == _sentinel ? this.currentIndex : currentIndex as int?,
      isPlaying: isPlaying ?? this.isPlaying,
      positionSecs: positionSecs ?? this.positionSecs,
      durationSecs: durationSecs ?? this.durationSecs,
      volume: volume ?? this.volume,
      mode: mode ?? this.mode,
      isBusy: isBusy ?? this.isBusy,
      error: error == _sentinel ? this.error : error as String?,
    );
  }
}
