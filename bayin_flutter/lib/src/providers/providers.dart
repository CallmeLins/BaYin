/// Riverpod providers.
///
/// Directory layout will grow as phases progress:
/// - `platform_provider.dart` (Phase 1): OS / isMobile / isDesktop / insets
/// - `responsive_provider.dart` (Phase 1): breakpoint / playerMode / sidebarMode
/// - `settings_provider.dart` (Phase 1): persisted user settings
/// - `theme_provider.dart` (Phase 1): ThemeMode (system/light/dark)
/// - `library_provider.dart` (Phase 3): songs / albums / artists lists
/// - `scanner_provider.dart` (Phase 3): scan progress + trigger
/// - `player_provider.dart` (Phase 4): current song / queue / progress / mode
/// - `lyrics_provider.dart` (Phase 5): parsed LRC / active line index
/// - `spectrum_provider.dart` (Phase 5): FFT data stream from Rust
/// - `eq_provider.dart` (Phase 7): 10-band gains
/// - `stream_servers_provider.dart` (Phase 6): Subsonic/Jellyfin servers
library;

export 'library_provider.dart';
export 'player_provider.dart';
export 'platform_provider.dart';
export 'responsive_provider.dart';
export 'scanner_provider.dart';
export 'settings_provider.dart';
