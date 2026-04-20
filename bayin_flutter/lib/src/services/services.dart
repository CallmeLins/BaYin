/// Dart-side service wrappers.
///
/// Thin Dart wrappers around Rust FFI calls and Flutter packages.
///
/// Directory layout will grow as phases progress:
/// - `media_session_service.dart` (Phase 4): audio_service adapter to Rust playback
/// - `window_service.dart` (Phase 2): window_manager + macos_ui adapter
/// - `storage_service.dart` (Phase 1): shared_preferences / hive wrapper
/// - `update_service.dart` (Phase 7): self-update check + apply
library;

export 'library_service.dart';
export 'media_session_service.dart';
export 'scan_service.dart';
export 'settings_service.dart';
export 'window_service.dart';
