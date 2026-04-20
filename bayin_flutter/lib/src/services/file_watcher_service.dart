import 'dart:async';

import '../rust/rust_api.dart';

class FileWatcherService {
  FileWatcherService._();

  static final FileWatcherService instance = FileWatcherService._();

  final StreamController<List<RustFileWatchEvent>> _eventsController =
      StreamController<List<RustFileWatchEvent>>.broadcast();

  Timer? _pollTimer;
  bool _running = false;

  Stream<List<RustFileWatchEvent>> get events => _eventsController.stream;

  Future<void> start(List<String> directories) async {
    if (directories.isEmpty) {
      await stop();
      return;
    }
    RustApi.instance.startFileWatcher(directories);
    _running = true;
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(
      const Duration(milliseconds: 1200),
      (_) => _pollOnce(),
    );
    await _pollOnce();
  }

  Future<void> stop() async {
    _pollTimer?.cancel();
    _pollTimer = null;
    if (_running) {
      RustApi.instance.stopFileWatcher();
    }
    _running = false;
  }

  RustFileWatcherStatus status() {
    return RustApi.instance.fileWatcherStatus();
  }

  Future<void> _pollOnce() async {
    try {
      final events = RustApi.instance.pollFileWatcherEvents();
      if (events.isNotEmpty) {
        _eventsController.add(events);
      }
    } catch (_) {
      // Keep polling resilient if watcher is unavailable on current platform.
    }
  }
}
