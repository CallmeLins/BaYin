import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../services/scan_service.dart';
import 'library_provider.dart';

class ScannerState {
  const ScannerState({
    this.results = const <ScannedSong>[],
    this.directories = const <String>[],
    this.isLoading = false,
    this.lastSave,
    this.error,
  });

  final List<ScannedSong> results;
  final List<String> directories;
  final bool isLoading;
  final ScanAndSaveResult? lastSave;
  final String? error;

  ScannerState copyWith({
    List<ScannedSong>? results,
    List<String>? directories,
    bool? isLoading,
    ScanAndSaveResult? lastSave,
    String? error,
    bool clearError = false,
    bool clearLastSave = false,
  }) {
    return ScannerState(
      results: results ?? this.results,
      directories: directories ?? this.directories,
      isLoading: isLoading ?? this.isLoading,
      lastSave: clearLastSave ? null : lastSave ?? this.lastSave,
      error: clearError ? null : error ?? this.error,
    );
  }
}

class ScannerController extends StateNotifier<ScannerState> {
  ScannerController(this._service, this._ref) : super(const ScannerState());

  final ScanService _service;
  final Ref _ref;

  Future<void> scanDirectories(
    List<String> directories, {
    bool skipShortAudio = false,
    double? minDuration,
  }) async {
    state = state.copyWith(
      directories: directories,
      isLoading: true,
      clearError: true,
    );

    try {
      final results = await _service.scanDirectories(
        directories,
        skipShortAudio: skipShortAudio,
        minDuration: minDuration,
      );
      state = state.copyWith(
        results: results,
        directories: directories,
        isLoading: false,
        clearError: true,
      );
    } catch (error) {
      state = state.copyWith(
        directories: directories,
        isLoading: false,
        error: error.toString(),
      );
    }
  }

  /// Scan + save + refresh library providers so SongsPage / Albums / Artists
  /// pick up new content without a manual reload.
  Future<void> scanAndSave(
    List<String> directories, {
    bool skipShortAudio = false,
    double? minDuration,
  }) async {
    state = state.copyWith(
      directories: directories,
      isLoading: true,
      clearError: true,
      clearLastSave: true,
    );

    try {
      final summary = await _service.scanAndSave(
        directories,
        skipShortAudio: skipShortAudio,
        minDuration: minDuration,
      );
      state = state.copyWith(
        directories: directories,
        isLoading: false,
        lastSave: summary,
        clearError: true,
      );
      _invalidateLibrary();
    } catch (error) {
      state = state.copyWith(
        directories: directories,
        isLoading: false,
        error: error.toString(),
      );
    }
  }

  Future<void> clearLibrary() async {
    await _service.clearLocalLibrary();
    state = state.copyWith(
      lastSave: const ScanAndSaveResult(scanned: 0, saved: 0, skipped: 0),
      clearError: true,
    );
    _invalidateLibrary();
  }

  void _invalidateLibrary() {
    _ref.invalidate(librarySongsProvider);
    _ref.invalidate(libraryAlbumsProvider);
    _ref.invalidate(libraryArtistsProvider);
  }
}

final scanServiceProvider = Provider<ScanService>((ref) {
  return ScanService.instance;
});

final scannerProvider =
    StateNotifierProvider<ScannerController, ScannerState>((ref) {
  final service = ref.watch(scanServiceProvider);
  return ScannerController(service, ref);
});

final scanConfigProvider = FutureProvider<ScanConfig?>((ref) async {
  final service = ref.watch(scanServiceProvider);
  return service.loadScanConfig();
});
