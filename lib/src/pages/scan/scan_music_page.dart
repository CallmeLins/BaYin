import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../theme/bayin_tokens.dart';
import '../../utils/info_bar_helper.dart';
import '../../rust/rust_api.dart';
import '../../services/file_watcher_service.dart';
import '../../services/scan_service.dart';
import '../../widgets/widgets.dart';
import 'folder_browser.dart';

class ScanMusicPage extends ConsumerStatefulWidget {
  const ScanMusicPage({super.key});

  @override
  ConsumerState<ScanMusicPage> createState() => _ScanMusicPageState();
}

class _ScanMusicPageState extends ConsumerState<ScanMusicPage> {
  final TextEditingController _pathController = TextEditingController();
  StreamSubscription<List<RustFileWatchEvent>>? _watchSubscription;
  List<String> _directories = <String>[];
  bool _skipShort = false;
  double _minDuration = 30;
  bool _configApplied = false;
  bool _watchRunning = false;
  int _watchPendingEvents = 0;
  int _watchEventCount = 0;
  String? _lastWatchEventPath;
  String? _lastWatchEventKind;

  @override
  void initState() {
    super.initState();
    _watchSubscription = FileWatcherService.instance.events.listen((events) {
      if (!mounted || events.isEmpty) return;
      final last = events.last;
      setState(() {
        _watchEventCount += events.length;
        _lastWatchEventPath = last.path;
        _lastWatchEventKind = last.kind;
        _watchPendingEvents = 0;
      });
      _refreshWatcherStatus();
    });
    _refreshWatcherStatus();
  }

  @override
  void dispose() {
    _watchSubscription?.cancel();
    _pathController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scanner = ref.watch(scannerProvider);
    final config = ref.watch(scanConfigProvider);
    final tokens = ref.watch(bayinTokensProvider);
    _applyConfigOnce(config);

    final surfaceBg = tokens.isDark
        ? Colors.white.withValues(alpha: 0.06)
        : Colors.black.withValues(alpha: 0.04);

    return Column(
      children: [
        const BayinPageHeader(
          title: Text('Scan Music'),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 2),
              _DirectoriesCard(
                directories: _directories,
                pathController: _pathController,
                surfaceBg: surfaceBg,
                tokens: tokens,
                onAddTyped: _addTypedPath,
                onBrowse: _browseFolder,
                onRemove: _removeDirectory,
              ),
              const SizedBox(height: 10),
              _ScanOptionsCard(
                skipShort: _skipShort,
                minDuration: _minDuration,
                surfaceBg: surfaceBg,
                onSkipShortChanged: (value) => setState(() => _skipShort = value),
                onMinDurationChanged: (value) => setState(() => _minDuration = value),
              ),
              const SizedBox(height: 10),
              _ActionCard(
                scanDisabled: scanner.isLoading || _directories.isEmpty,
                backfillDisabled: scanner.isLoading,
                surfaceBg: surfaceBg,
                onSaveConfig: _saveConfig,
                onPreviewScan: _previewScan,
                onScanAndSave: _scanAndSave,
                onBackfillCovers: _backfillCovers,
              ),
              if (scanner.isLoading) ...[
                const SizedBox(height: 10),
                const LinearProgressIndicator(),
              ],
              const SizedBox(height: 10),
              _ResultCard(
                scannedCount: scanner.results.length,
                directories: scanner.directories,
                lastSave: scanner.lastSave,
                error: scanner.error,
                surfaceBg: surfaceBg,
                tokens: tokens,
                watchRunning: _watchRunning,
                watchPendingEvents: _watchPendingEvents,
                watchEventCount: _watchEventCount,
                lastWatchEventPath: _lastWatchEventPath,
                lastWatchEventKind: _lastWatchEventKind,
              ),
              if (scanner.results.isNotEmpty) ...[
                const SizedBox(height: 10),
                _PreviewSongsCard(scanner.results, surfaceBg: surfaceBg, tokens: tokens),
              ],
            ],
          ),
        ),
      ],
    );
  }

  void _applyConfigOnce(AsyncValue<ScanConfig?> config) {
    if (_configApplied) return;
    if (!config.hasValue) return;
    _configApplied = true;
    final data = config.valueOrNull;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (data == null) return;
      setState(() {
        _directories = List<String>.from(data.directories);
        _skipShort = data.skipShort;
        _minDuration = data.minDuration;
      });
    });
  }

  Future<void> _browseFolder() async {
    final initialPath =
        _directories.isNotEmpty ? _directories.last : Directory.current.path;
    final selected = await FolderBrowser.show(context, initialPath: initialPath);
    if (selected == null || selected.trim().isEmpty) return;
    _addDirectory(selected.trim());
  }

  void _addTypedPath() {
    final path = _pathController.text.trim();
    if (path.isEmpty) return;
    _addDirectory(path);
    _pathController.clear();
  }

  void _addDirectory(String path) {
    if (_directories.contains(path)) return;
    setState(() {
      _directories = <String>[..._directories, path];
    });
  }

  void _removeDirectory(String path) {
    setState(() {
      _directories = _directories.where((item) => item != path).toList();
    });
  }

  Future<void> _saveConfig() async {
    try {
      await ref.read(scanServiceProvider).saveScanConfig(
            ScanConfig(
              directories: List<String>.unmodifiable(_directories),
              skipShort: _skipShort,
              minDuration: _minDuration,
              lastScanAt: DateTime.now().millisecondsSinceEpoch,
            ),
          );
      await FileWatcherService.instance.start(_directories);
      _refreshWatcherStatus();
      ref.invalidate(scanConfigProvider);
      if (!mounted) return;
      showInfoMessage(context, 'Scan config saved.');
    } catch (error) {
      if (!mounted) return;
      showInfoMessage(context, 'Failed to save config: $error', isError: true);
    }
  }

  void _refreshWatcherStatus() {
    try {
      final status = FileWatcherService.instance.status();
      if (!mounted) return;
      setState(() {
        _watchRunning = status.running;
        _watchPendingEvents = status.pendingEvents;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _watchRunning = false;
        _watchPendingEvents = 0;
      });
    }
  }

  Future<void> _previewScan() async {
    await _saveConfig();
    await ref.read(scannerProvider.notifier).scanDirectories(
          _directories,
          skipShortAudio: _skipShort,
          minDuration: _minDuration,
        );
  }

  Future<void> _scanAndSave() async {
    await _saveConfig();
    await ref.read(scannerProvider.notifier).scanAndSave(
          _directories,
          skipShortAudio: _skipShort,
          minDuration: _minDuration,
        );
  }

  Future<void> _backfillCovers() async {
    try {
      await ref.read(libraryServiceProvider).ensureDatabaseInitialized();
      final result = await ref.read(scanServiceProvider).backfillLocalCovers();
      ref.invalidate(librarySongsProvider);
      ref.invalidate(libraryAlbumsProvider);
      ref.invalidate(libraryArtistsProvider);
      if (!mounted) return;
      showInfoMessage(
          context,
          'Cover backfill finished. Updated ${result.updated}/${result.totalCandidates}, skipped ${result.skipped}, failed ${result.failed}.',
          duration: const Duration(seconds: 5));
    } catch (error) {
      if (!mounted) return;
      showInfoMessage(context, 'Cover backfill failed: $error', isError: true);
    }
  }
}

class _DirectoriesCard extends StatelessWidget {
  const _DirectoriesCard({
    required this.directories,
    required this.pathController,
    required this.surfaceBg,
    required this.tokens,
    required this.onAddTyped,
    required this.onBrowse,
    required this.onRemove,
  });

  final List<String> directories;
  final TextEditingController pathController;
  final Color surfaceBg;
  final BayinTokens tokens;
  final VoidCallback onAddTyped;
  final VoidCallback onBrowse;
  final ValueChanged<String> onRemove;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: surfaceBg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text('Folders', style: TextStyle(fontWeight: FontWeight.w600)),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: pathController,
                  decoration: const InputDecoration(
                    hintText: r'Enter a folder path, e.g. D:\Music',
                    border: InputBorder.none,
                  ),
                  onSubmitted: (value) => onAddTyped(),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                onPressed: onAddTyped,
                icon: Icon(PhosphorIcons.plus()),
              ),
              const SizedBox(width: 8),
              IconButton(
                onPressed: onBrowse,
                icon: Icon(PhosphorIcons.folderOpen()),
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (directories.isEmpty)
            Text('No folders selected yet.',
                style: TextStyle(fontSize: 12, color: tokens.textSecondary))
          else
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final path in directories)
                  _PathChip(path: path, onRemove: () => onRemove(path)),
              ],
            ),
        ],
      ),
    );
  }
}

class _PathChip extends StatelessWidget {
  const _PathChip({required this.path, required this.onRemove});
  final String path;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.only(left: 10, right: 4, top: 6, bottom: 6),
      decoration: BoxDecoration(
        color: const Color(0x0A000000),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(path,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 13)),
          const SizedBox(width: 4),
          GestureDetector(
            onTap: onRemove,
            child: Icon(PhosphorIcons.x(), size: 16),
          ),
        ],
      ),
    );
  }
}

class _ScanOptionsCard extends StatelessWidget {
  const _ScanOptionsCard({
    required this.skipShort,
    required this.minDuration,
    required this.surfaceBg,
    required this.onSkipShortChanged,
    required this.onMinDurationChanged,
  });

  final bool skipShort;
  final double minDuration;
  final Color surfaceBg;
  final ValueChanged<bool> onSkipShortChanged;
  final ValueChanged<double> onMinDurationChanged;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
      decoration: BoxDecoration(
        color: surfaceBg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('Skip short tracks',
                        style: TextStyle(fontWeight: FontWeight.w600)),
                    const Text('Ignore tracks shorter than the minimum duration.',
                        style: TextStyle(fontSize: 12)),
                  ],
                ),
              ),
              Switch(value: skipShort, onChanged: onSkipShortChanged),
            ],
          ),
          const SizedBox(height: 6),
          Text('Minimum duration: ${minDuration.round()}s',
              style: const TextStyle(fontWeight: FontWeight.w600)),
          Slider(
            value: minDuration.clamp(5, 300),
            min: 5,
            max: 300,
            divisions: 59,
            label: '${minDuration.round()}s',
            onChanged: onMinDurationChanged,
          ),
        ],
      ),
    );
  }
}

class _ActionCard extends StatelessWidget {
  const _ActionCard({
    required this.scanDisabled,
    required this.backfillDisabled,
    required this.surfaceBg,
    required this.onSaveConfig,
    required this.onPreviewScan,
    required this.onScanAndSave,
    required this.onBackfillCovers,
  });

  final bool scanDisabled;
  final bool backfillDisabled;
  final Color surfaceBg;
  final Future<void> Function() onSaveConfig;
  final Future<void> Function() onPreviewScan;
  final Future<void> Function() onScanAndSave;
  final Future<void> Function() onBackfillCovers;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: surfaceBg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          SizedBox(
            width: double.infinity,
            child: OutlinedButton(
              onPressed: scanDisabled ? null : onSaveConfig,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(PhosphorIcons.floppyDisk()),
                  const SizedBox(width: 8),
                  const Text('Save scan config'),
                ],
              ),
            ),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: OutlinedButton(
                  onPressed: scanDisabled ? null : onPreviewScan,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(PhosphorIcons.magnifyingGlass()),
                      const SizedBox(width: 8),
                      const Text('Preview scan'),
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: FilledButton(
                  onPressed: scanDisabled ? null : onScanAndSave,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(PhosphorIcons.database()),
                      const SizedBox(width: 8),
                      const Text('Scan & save'),
                    ],
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          SizedBox(
            width: double.infinity,
            child: OutlinedButton(
              onPressed: backfillDisabled ? null : onBackfillCovers,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(PhosphorIcons.imageSquare()),
                  const SizedBox(width: 8),
                  const Text('Backfill covers'),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ResultCard extends StatelessWidget {
  const _ResultCard({
    required this.scannedCount,
    required this.directories,
    required this.lastSave,
    required this.error,
    required this.surfaceBg,
    required this.tokens,
    required this.watchRunning,
    required this.watchPendingEvents,
    required this.watchEventCount,
    required this.lastWatchEventPath,
    required this.lastWatchEventKind,
  });

  final int scannedCount;
  final List<String> directories;
  final ScanAndSaveResult? lastSave;
  final String? error;
  final Color surfaceBg;
  final BayinTokens tokens;
  final bool watchRunning;
  final int watchPendingEvents;
  final int watchEventCount;
  final String? lastWatchEventPath;
  final String? lastWatchEventKind;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: surfaceBg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text('Scan status',
              style: TextStyle(fontWeight: FontWeight.w600)),
          const SizedBox(height: 6),
          Text('Preview results: $scannedCount'),
          if (directories.isNotEmpty)
            Text('Last run: ${directories.join(', ')}',
                style: TextStyle(color: tokens.textSecondary)),
          if (lastSave != null)
            Text(
                'Saved: scanned ${lastSave!.scanned}, saved ${lastSave!.saved}, skipped ${lastSave!.skipped}',
                style: TextStyle(color: tokens.textSecondary)),
          Text('Watcher: ${watchRunning ? 'running' : 'stopped'}',
              style: TextStyle(color: tokens.textSecondary)),
          Text(
              'Watcher events: $watchEventCount (pending: $watchPendingEvents)',
              style: TextStyle(color: tokens.textSecondary)),
          if (lastWatchEventPath != null && lastWatchEventPath!.isNotEmpty)
            Text(
              'Last watcher event: ${lastWatchEventKind ?? 'unknown'} - $lastWatchEventPath',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: tokens.textSecondary),
            ),
          if (error != null)
            Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Text('Error: $error', style: TextStyle(color: Colors.red)),
            ),
        ],
      ),
    );
  }
}

class _PreviewSongsCard extends StatelessWidget {
  const _PreviewSongsCard(this.results,
      {required this.surfaceBg, required this.tokens});

  final List<ScannedSong> results;
  final Color surfaceBg;
  final BayinTokens tokens;

  @override
  Widget build(BuildContext context) {
    final maxCount = results.length > 20 ? 20 : results.length;
    return Container(
      decoration: BoxDecoration(
        color: surfaceBg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          const Padding(
            padding: EdgeInsets.fromLTRB(12, 10, 12, 8),
            child: Row(
              children: [
                Text('Preview songs',
                    style: TextStyle(fontWeight: FontWeight.w600)),
              ],
            ),
          ),
          for (var i = 0; i < maxCount; i++)
            Padding(
              padding:
                  const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(results[i].title,
                      maxLines: 1, overflow: TextOverflow.ellipsis),
                  Text('${results[i].artist} - ${results[i].album}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 12)),
                ],
              ),
            ),
          if (results.length > maxCount)
            Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Text(
                '...and ${results.length - maxCount} more',
                style: TextStyle(
                    fontSize: 12, color: tokens.textSecondary),
              ),
            ),
        ],
      ),
    );
  }
}
