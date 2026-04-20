/// Freezed data classes.
///
/// Directory layout will grow as phases progress:
/// - `song.dart` (Phase 1): Song / ScannedSong
/// - `album.dart` (Phase 1): Album
/// - `artist.dart` (Phase 1): Artist
/// - `playlist.dart` (Phase 1): Playlist
/// - `stream_server.dart` (Phase 1): StreamServerConfig
/// - `lyric_line.dart` (Phase 5): LyricLine + KaraokeToken
/// - `play_mode.dart` (Phase 4): enum PlayMode
/// - `spectrum_mode.dart` (Phase 5): enum SpectrumMode
library;

export 'album.dart';
export 'artist.dart';
export 'lyric_line.dart';
export 'play_mode.dart';
export 'playlist.dart';
export 'player_state.dart';
export 'scanned_song.dart';
export 'song.dart';
export 'spectrum_mode.dart';
export 'stream_server.dart';
