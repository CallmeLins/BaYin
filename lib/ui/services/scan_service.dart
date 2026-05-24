import 'dart:isolate';
import '../models/models.dart';
import '../rust/rust_api.dart';

class ScanAndSaveResult {
  const ScanAndSaveResult({
    required this.scanned,
    required this.saved,
    required this.skipped,
  });

  factory ScanAndSaveResult.fromRust(RustScanAndSaveResult value) =>
      ScanAndSaveResult(
        scanned: value.scanned,
        saved: value.saved,
        skipped: value.skipped,
      );

  final int scanned;
  final int saved;
  final int skipped;
}

class BackfillCoversResult {
  const BackfillCoversResult({
    required this.totalCandidates,
    required this.updated,
    required this.skipped,
    required this.failed,
  });

  factory BackfillCoversResult.fromRust(RustBackfillCoversResult value) {
    return BackfillCoversResult(
      totalCandidates: value.totalCandidates,
      updated: value.updated,
      skipped: value.skipped,
      failed: value.failed,
    );
  }

  final int totalCandidates;
  final int updated;
  final int skipped;
  final int failed;
}

class ScanConfig {
  const ScanConfig({
    required this.directories,
    required this.skipShort,
    required this.minDuration,
    this.lastScanAt,
  });

  factory ScanConfig.fromRust(RustScanConfig value) => ScanConfig(
        directories: List<String>.unmodifiable(value.directories),
        skipShort: value.skipShort,
        minDuration: value.minDuration,
        lastScanAt: value.lastScanAt,
      );

  final List<String> directories;
  final bool skipShort;
  final double minDuration;
  final int? lastScanAt;
}

class ScanService {
  ScanService._();

  static final ScanService instance = ScanService._();

  /// Scan only — returns results without touching the DB. Kept for the debug
  /// smoke test surface.
  Future<List<ScannedSong>> scanDirectories(
    List<String> directories, {
    bool skipShortAudio = false,
    double? minDuration,
  }) async {
    final results = await Isolate.run(() => RustApi.instance.scanMusicFiles(
      RustScanOptions(
        directories: directories,
        skipShortAudio: skipShortAudio,
        minDuration: minDuration,
      ),
    ));
    return results.map(ScannedSong.fromRust).toList();
  }

  /// Scan + persist to the local DB as source_type=local.
  Future<ScanAndSaveResult> scanAndSave(
    List<String> directories, {
    bool skipShortAudio = false,
    double? minDuration,
  }) async {
    final result = await Isolate.run(() => RustApi.instance.scanAndSaveMusicFiles(
      RustScanOptions(
        directories: directories,
        skipShortAudio: skipShortAudio,
        minDuration: minDuration,
      ),
    ));
    return ScanAndSaveResult.fromRust(result);
  }

  Future<void> clearLocalLibrary() async {
    await Isolate.run(() => RustApi.instance.clearAllSongs());
  }

  Future<BackfillCoversResult> backfillLocalCovers() async {
    final result = await Isolate.run(() => RustApi.instance.backfillSongCovers());
    return BackfillCoversResult.fromRust(result);
  }

  Future<void> saveScanConfig(ScanConfig config) async {
    await Isolate.run(() => RustApi.instance.saveScanConfig(
      RustScanConfig(
        directories: config.directories,
        skipShort: config.skipShort,
        minDuration: config.minDuration,
        lastScanAt: config.lastScanAt,
      ),
    ));
  }

  Future<ScanConfig?> loadScanConfig() async {
    final raw = await Isolate.run(() => RustApi.instance.getScanConfig());
    if (raw == null) return null;
    return ScanConfig.fromRust(raw);
  }
}
