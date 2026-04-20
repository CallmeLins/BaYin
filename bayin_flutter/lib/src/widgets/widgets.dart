/// Reusable widgets (non-route-level).
///
/// Directory layout will grow as phases progress:
/// - `root_scaffold.dart` (Phase 2): shell with titlebar + sidebar + player_bar
/// - `sidebar.dart` (Phase 2): navigation drawer
/// - `player_bar.dart` (Phase 2): bottom playback strip
/// - `page_header.dart` (Phase 2): sticky page header with drag region
/// - `song_list.dart` (Phase 3): virtualized song list
/// - `alphabet_scroller.dart` (Phase 3): pinyin index scrubber
/// - `player_stage.dart` (Phase 5): cover + spectrum composite
/// - `karaoke_line.dart` (Phase 5): per-word highlighting
/// - `spectrum/*.dart` (Phase 5): 8 CustomPainter modes
/// - `equalizer_panel.dart` (Phase 7): 10-band EQ sliders
library;

export 'alphabet_scroller.dart';
export 'placeholder_page.dart';
export 'player_bar.dart';
export 'root_scaffold.dart';
export 'sidebar.dart';
export 'song_list.dart';
export 'song_menu.dart';
export 'titlebar.dart';
