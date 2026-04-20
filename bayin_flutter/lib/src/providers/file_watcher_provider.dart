import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/rust_api.dart';
import '../services/file_watcher_service.dart';

final fileWatcherServiceProvider = Provider<FileWatcherService>((ref) {
  return FileWatcherService.instance;
});

final fileWatcherEventsProvider =
    StreamProvider<List<RustFileWatchEvent>>((ref) {
  final service = ref.watch(fileWatcherServiceProvider);
  return service.events;
});
