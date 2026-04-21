import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../rust/rust_api.dart';
import '../../services/file_watcher_service.dart';
import '../../services/scan_service.dart';
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
    _applyConfigOnce(config);

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'Scan Music',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 10),
        _DirectoriesCard(
          directories: _directories,
          pathController: _pathController,
          onAddTyped: _addTypedPath,
          onBrowse: _browseFolder,
          onRemove: _removeDirectory,
        ),
        const SizedBox(height: 10),
        _ScanOptionsCard(
          skipShort: _skipShort,
          minDuration: _minDuration,
          onSkipShortChanged: (value) => setState(() => _skipShort = value),
          onMinDurationChanged: (value) => setState(() => _minDuration = value),
        ),
        const SizedBox(height: 10),
        _ActionCard(
          scanDisabled: scanner.isLoading || _directories.isEmpty,
          backfillDisabled: scanner.isLoading,
          onSaveConfig: _saveConfig,
          onPreviewScan: _previewScan,
          onScanAndSave: _scanAndSave,
          onBackfillCovers: _backfillCovers,
        ),
        if (scanner.isLoading) ...[
          const SizedBox(height: 10),
          const LinearProgressIndicator(minHeight: 2),
        ],
        const SizedBox(height: 10),
        _ResultCard(
          scannedCount: scanner.results.length,
          directories: scanner.directories,
          lastSave: scanner.lastSave,
          error: scanner.error,
          watchRunning: _watchRunning,
          watchPendingEvents: _watchPendingEvents,
          watchEventCount: _watchEventCount,
          lastWatchEventPath: _lastWatchEventPath,
          lastWatchEventKind: _lastWatchEventKind,
        ),
        if (scanner.results.isNotEmpty) ...[
          const SizedBox(height: 10),
          _PreviewSongsCard(scanner.results),
        ],
      ],
    );
  }

  void _applyConfigOnce(AsyncValue<ScanConfig?> config) {
    if (_configApplied) {
      return;
    }
    if (!config.hasValue) {
      return;
    }
    _configApplied = true;
    final data = config.valueOrNull;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      if (data == null) {
        return;
      }
      setState(() {
        _directories = List<String>.from(data.directories);
        _skipShort = data.skipShort;
        _minDuration = data.minDuration;
      });
    });
  }

  Future<void> _browseFolder() async {
    final initialPath = _directories.isNotEmpty
        ? _directories.last
        : Directory.current.path;
    final selected = await FolderBrowser.show(
      context,
      initialPath: initialPath,
    );
    if (selected == null || selected.trim().isEmpty) {
      return;
    }
    _addDirectory(selected.trim());
  }

  void _addTypedPath() {
    final path = _pathController.text.trim();
    if (path.isEmpty) {
      return;
    }
    _addDirectory(path);
    _pathController.clear();
  }

  void _addDirectory(String path) {
    if (_directories.contains(path)) {
      return;
    }
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
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Scan config saved.')),
      );
    } catch (error) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Failed to save config: $error')),
      );
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Cover backfill finished. Updated ${result.updated}/${result.totalCandidates}, skipped ${result.skipped}, failed ${result.failed}.',
          ),
        ),
      );
    } catch (error) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Cover backfill failed: $error')),
      );
    }
  }
}

class _DirectoriesCard extends StatelessWidget {
  const _DirectoriesCard({
    required this.directories,
    required this.pathController,
    required this.onAddTyped,
    required this.onBrowse,
    required this.onRemove,
  });

  final List<String> directories;
  final TextEditingController pathController;
  final VoidCallback onAddTyped;
  final VoidCallback onBrowse;
  final ValueChanged<String> onRemove;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Folders',
            style: TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: pathController,
                  decoration: const InputDecoration(
                    border: OutlineInputBorder(),
                    isDense: true,
                    hintText: r'Enter a folder path, e.g. D:\Music',
                  ),
                  onSubmitted: (_) => onAddTyped(),
                ),
              ),
              const SizedBox(width: 8),
              IconButton.filledTonal(
                onPressed: onAddTyped,
                icon: const Icon(Icons.add),
                tooltip: 'Add path',
              ),
              const SizedBox(width: 8),
              IconButton.filled(
                onPressed: onBrowse,
                icon: Icon(PhosphorIcons.folderOpen()),
                tooltip: 'Browse folders',
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (directories.isEmpty)
            Text(
              'No folders selected yet.',
              style: TextStyle(
                fontSize: 12,
                color: scheme.onSurfaceVariant,
              ),
            )
          else
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final path in directories)
                  InputChip(
                    label: Text(path, overflow: TextOverflow.ellipsis),
                    onDeleted: () => onRemove(path),
                  ),
              ],
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
    required this.onSkipShortChanged,
    required this.onMinDurationChanged,
  });

  final bool skipShort;
  final double minDuration;
  final ValueChanged<bool> onSkipShortChanged;
  final ValueChanged<double> onMinDurationChanged;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SwitchListTile.adaptive(
            contentPadding: EdgeInsets.zero,
            value: skipShort,
            title: const Text('Skip short tracks'),
            subtitle: const Text('Ignore tracks shorter than the minimum duration.'),
            onChanged: onSkipShortChanged,
          ),
          const SizedBox(height: 6),
          Text(
            'Minimum duration: ${minDuration.round()}s',
            style: const TextStyle(fontWeight: FontWeight.w600),
          ),
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
    required this.onSaveConfig,
    required this.onPreviewScan,
    required this.onScanAndSave,
    required this.onBackfillCovers,
  });

  final bool scanDisabled;
  final bool backfillDisabled;
  final Future<void> Function() onSaveConfig;
  final Future<void> Function() onPreviewScan;
  final Future<void> Function() onScanAndSave;
  final Future<void> Function() onBackfillCovers;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          SizedBox(
            width: double.infinity,
            child: OutlinedButton.icon(
              onPressed: scanDisabled ? null : onSaveConfig,
              icon: const Icon(Icons.save_alt),
              label: const Text('Save scan config'),
            ),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: OutlinedButton.icon(
                  onPressed: scanDisabled ? null : onPreviewScan,
                  icon: Icon(PhosphorIcons.magnifyingGlass()),
                  label: const Text('Preview scan'),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: FilledButton.icon(
                  onPressed: scanDisabled ? null : onScanAndSave,
                  icon: Icon(PhosphorIcons.database()),
                  label: const Text('Scan & save'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          SizedBox(
            width: double.infinity,
            child: OutlinedButton.icon(
              onPressed: backfillDisabled ? null : onBackfillCovers,
              icon: Icon(PhosphorIcons.imageSquare()),
              label: const Text('Backfill covers'),
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
  final bool watchRunning;
  final int watchPendingEvents;
  final int watchEventCount;
  final String? lastWatchEventPath;
  final String? lastWatchEventKind;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Scan status',
            style: TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 6),
          Text('Preview results: $scannedCount'),
          if (directories.isNotEmpty)
            Text(
              'Last run: ${directories.join(', ')}',
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          if (lastSave != null)
            Text(
              'Saved: scanned ${lastSave!.scanned}, saved ${lastSave!.saved}, skipped ${lastSave!.skipped}',
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          Text(
            'Watcher: ${watchRunning ? 'running' : 'stopped'}',
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          Text(
            'Watcher events: $watchEventCount (pending: $watchPendingEvents)',
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          if (lastWatchEventPath != null && lastWatchEventPath!.isNotEmpty)
            Text(
              'Last watcher event: ${lastWatchEventKind ?? 'unknown'} - $lastWatchEventPath',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          if (error != null)
            Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Text(
                'Error: $error',
                style: TextStyle(color: scheme.error),
              ),
            ),
        ],
      ),
    );
  }
}

class _PreviewSongsCard extends StatelessWidget {
  const _PreviewSongsCard(this.results);

  final List<ScannedSong> results;

  @override
  Widget build(BuildContext context) {
    final maxCount = results.length > 20 ? 20 : results.length;
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          const Padding(
            padding: EdgeInsets.fromLTRB(12, 10, 12, 8),
            child: Row(
              children: [
                Text(
                  'Preview songs',
                  style: TextStyle(fontWeight: FontWeight.w600),
                ),
              ],
            ),
          ),
          for (var i = 0; i < maxCount; i++)
            ListTile(
              dense: true,
              visualDensity: VisualDensity.compact,
              title: Text(
                results[i].title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              subtitle: Text(
                '${results[i].artist} - ${results[i].album}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          if (results.length > maxCount)
            Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Text(
                '...and ${results.length - maxCount} more',
                style: TextStyle(
                  fontSize: 12,
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ),
        ],
      ),
    );
  }
}
