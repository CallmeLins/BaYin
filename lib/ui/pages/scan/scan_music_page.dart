import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
import '../../theme/macos_design_tokens.dart';
import '../../utils/info_bar_helper.dart';
import '../../rust/rust_api.dart';
import '../../services/file_watcher_service.dart';
import '../../services/scan_service.dart';
import '../../widgets/widgets.dart';
import '../../widgets/glass/glass.dart';
import 'folder_browser.dart';

/// Scan Music Page — macOS-style glassmorphism UI.
///
/// Features:
/// - Glass material cards with Retina borders
/// - Traffic lights in header
/// - Segmented controls for scan options
/// - Animated progress indicators
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

    return Column(
      children: [
        // ── Header ───────────────────────────────────────────────────────
        const BayinPageHeader(
          title: Text('Scan Music'),
        ),

        // ── Content ──────────────────────────────────────────────────────
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),

              // ── Folders Card ─────────────────────────────────────────
              _GlassCard(
                title: 'Folders',
                icon: PhosphorIcons.folder(),
                child: _DirectoriesSection(
                  directories: _directories,
                  pathController: _pathController,
                  onAddTyped: _addTypedPath,
                  onBrowse: _browseFolder,
                  onRemove: _removeDirectory,
                ),
              ),

              const SizedBox(height: 16),

              // ── Scan Options Card ────────────────────────────────────
              _GlassCard(
                title: 'Options',
                icon: PhosphorIcons.gear(),
                child: _ScanOptionsSection(
                  skipShort: _skipShort,
                  minDuration: _minDuration,
                  onSkipShortChanged: (value) =>
                      setState(() => _skipShort = value),
                  onMinDurationChanged: (value) =>
                      setState(() => _minDuration = value),
                ),
              ),

              const SizedBox(height: 16),

              // ── Actions Card ─────────────────────────────────────────
              _GlassCard(
                title: 'Actions',
                icon: PhosphorIcons.playCircle(),
                child: _ActionsSection(
                  scanDisabled: scanner.isLoading || _directories.isEmpty,
                  backfillDisabled: scanner.isLoading,
                  onSaveConfig: _saveConfig,
                  onPreviewScan: _previewScan,
                  onScanAndSave: _scanAndSave,
                  onBackfillCovers: _backfillCovers,
                ),
              ),

              // ── Progress Indicator ───────────────────────────────────
              if (scanner.isLoading) ...[
                const SizedBox(height: 16),
                const _ScanProgressIndicator(),
              ],

              const SizedBox(height: 16),

              // ── Status Card ──────────────────────────────────────────
              _GlassCard(
                title: 'Status',
                icon: PhosphorIcons.chartBar(),
                child: _StatusSection(
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
              ),

              // ── Preview Songs ────────────────────────────────────────
              if (scanner.results.isNotEmpty) ...[
                const SizedBox(height: 16),
                _GlassCard(
                  title: 'Preview',
                  icon: PhosphorIcons.musicNote(),
                  child: _PreviewSongsSection(scanner.results),
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }

  // ── Private Methods ────────────────────────────────────────────────────

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

// ═══════════════════════════════════════════════════════════════════════════
// Glass Card Container
// ═══════════════════════════════════════════════════════════════════════════

/// Reusable glass card with title and icon.
class _GlassCard extends StatelessWidget {
  const _GlassCard({
    required this.title,
    required this.icon,
    required this.child,
  });

  final String title;
  final IconData icon;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return BayinGlassGroup(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Section header.
            Row(
              children: [
                Icon(
                  icon,
                  size: 16,
                  color: FlatColors.textSecondary(brightness),
                ),
                const SizedBox(width: 8),
                Text(
                  title,
                  style: FlatTypography.label(brightness).copyWith(
                    color: FlatColors.textSecondary(brightness),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Divider(
                    height: 1,
                    thickness: 1,
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.10)
                        : Colors.black.withValues(alpha: 0.06),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),

            // Content.
            child,
          ],
        ),
      ),
    );
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// Directories Section
// ═══════════════════════════════════════════════════════════════════════════

class _DirectoriesSection extends StatelessWidget {
  const _DirectoriesSection({
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
    final brightness = Theme.of(context).brightness;
    final isDark = brightness == Brightness.dark;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // ── Styled Input Container ─────────────────────────────────
        Container(
          decoration: BoxDecoration(
            color: isDark
                ? Colors.white.withValues(alpha: 0.04)
                : Colors.black.withValues(alpha: 0.03),
            borderRadius: BorderRadius.circular(10),
            border: Border.all(
              color: isDark
                  ? Colors.white.withValues(alpha: 0.08)
                  : Colors.black.withValues(alpha: 0.08),
              width: 0.5,
            ),
          ),
          child: Column(
            children: [
              // ── Input Row ───────────────────────────────────────
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                child: Row(
                  children: [
                    // Folder icon.
                    Icon(
                      PhosphorIcons.folderSimple(),
                      size: 18,
                      color: FlatColors.primary(brightness),
                    ),
                    const SizedBox(width: 10),

                    // Text field.
                    Expanded(
                      child: TextField(
                        controller: pathController,
                        decoration: InputDecoration(
                          hintText: 'D:\\Music',
                          hintStyle: FlatTypography.bodySmall(brightness).copyWith(
                            color: FlatColors.textTertiary(brightness),
                          ),
                          border: InputBorder.none,
                          contentPadding: const EdgeInsets.symmetric(vertical: 8),
                          isDense: true,
                        ),
                        style: FlatTypography.bodySmall(brightness),
                        onSubmitted: (_) => onAddTyped(),
                      ),
                    ),

                    // Browse button.
                    _PillButton(
                      label: 'Browse',
                      icon: PhosphorIcons.folderOpen(),
                      onPressed: onBrowse,
                    ),
                  ],
                ),
              ),

              // ── Divider ──────────────────────────────────────────
              Container(
                height: 1,
                color: isDark
                    ? Colors.white.withValues(alpha: 0.06)
                    : Colors.black.withValues(alpha: 0.06),
              ),

              // ── Action Bar ───────────────────────────────────────
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                child: Row(
                  children: [
                    Icon(
                      PhosphorIcons.info(),
                      size: 12,
                      color: FlatColors.textTertiary(brightness),
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        directories.isEmpty
                            ? 'Select music folders to scan'
                            : '${directories.length} folder${directories.length > 1 ? 's' : ''} selected',
                        style: FlatTypography.caption(brightness),
                      ),
                    ),
                    // Add button.
                    _AddButton(
                      onPressed: pathController.text.trim().isEmpty ? null : onAddTyped,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),

        const SizedBox(height: 12),

        // ── Directory Chips ────────────────────────────────────────
        if (directories.isEmpty)
          _EmptyState(
            icon: PhosphorIcons.folderOpen(),
            message: 'No folders added yet',
          )
        else
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              for (final path in directories)
                _DirectoryChip(
                  path: path,
                  onRemove: () => onRemove(path),
                ),
            ],
          ),
      ],
    );
  }
}

/// Pill-shaped button for the input area.
class _PillButton extends StatelessWidget {
  const _PillButton({
    required this.label,
    required this.icon,
    required this.onPressed,
  });

  final String label;
  final IconData icon;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final isDark = brightness == Brightness.dark;

    return Material(
      color: isDark
          ? Colors.white.withValues(alpha: 0.08)
          : Colors.black.withValues(alpha: 0.06),
      borderRadius: BorderRadius.circular(6),
      child: InkWell(
        onTap: onPressed,
        borderRadius: BorderRadius.circular(6),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 14, color: FlatColors.textSecondary(brightness)),
              const SizedBox(width: 5),
              Text(
                label,
                style: FlatTypography.caption(brightness).copyWith(
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Add button with icon.
class _AddButton extends StatelessWidget {
  const _AddButton({required this.onPressed});

  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final isEnabled = onPressed != null;

    return Material(
      color: isEnabled
          ? FlatColors.primary(brightness)
          : FlatColors.stateLayer(brightness, FlatStateIntensity.subtle),
      borderRadius: BorderRadius.circular(6),
      child: InkWell(
        onTap: onPressed,
        borderRadius: BorderRadius.circular(6),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                PhosphorIcons.plus(),
                size: 14,
                color: isEnabled ? Colors.white : FlatColors.textTertiary(brightness),
              ),
              const SizedBox(width: 4),
              Text(
                'Add',
                style: FlatTypography.caption(brightness).copyWith(
                  color: isEnabled ? Colors.white : FlatColors.textTertiary(brightness),
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// Scan Options Section
// ═══════════════════════════════════════════════════════════════════════════

class _ScanOptionsSection extends StatelessWidget {
  const _ScanOptionsSection({
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
    final brightness = Theme.of(context).brightness;

    return Column(
      children: [
        // Skip short tracks toggle.
        Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Skip short tracks',
                    style: FlatTypography.bodySmall(brightness).copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    'Ignore tracks shorter than minimum duration',
                    style: FlatTypography.caption(brightness),
                  ),
                ],
              ),
            ),
            MacosSwitch(
              value: skipShort,
              onChanged: onSkipShortChanged,
            ),
          ],
        ),

        const SizedBox(height: 16),

        // Duration slider.
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  PhosphorIcons.clock(),
                  size: 14,
                  color: FlatColors.textSecondary(brightness),
                ),
                const SizedBox(width: 6),
                Text(
                  'Minimum duration',
                  style: FlatTypography.bodySmall(brightness).copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const Spacer(),
                MacosBadge(
                  label: '${minDuration.round()}s',
                  color: FlatColors.primary(brightness),
                ),
              ],
            ),
            const SizedBox(height: 8),
            MacosSlider(
              value: minDuration.clamp(5.0, 300.0),
              min: 5,
              max: 300,
              divisions: 59,
              onChanged: onMinDurationChanged,
            ),
          ],
        ),
      ],
    );
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// Actions Section
// ═══════════════════════════════════════════════════════════════════════════

class _ActionsSection extends StatelessWidget {
  const _ActionsSection({
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
    final outlinedStyle = BayinButtonStyles.outlinedFlat(context);
    final filledStyle = BayinButtonStyles.filledFlat();

    return Column(
      children: [
        // Save config button.
        SizedBox(
          width: double.infinity,
          child: OutlinedButton.icon(
            onPressed: scanDisabled ? null : onSaveConfig,
            style: outlinedStyle,
            icon: Icon(PhosphorIcons.floppyDisk(), size: 16),
            label: const Text('Save configuration'),
          ),
        ),

        const SizedBox(height: 8),

        // Scan buttons row.
        Row(
          children: [
            Expanded(
              child: OutlinedButton.icon(
                onPressed: scanDisabled ? null : onPreviewScan,
                style: outlinedStyle,
                icon: Icon(PhosphorIcons.magnifyingGlass(), size: 16),
                label: const Text('Preview'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: FilledButton(
                onPressed: scanDisabled ? null : onScanAndSave,
                style: filledStyle,
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(PhosphorIcons.database(), size: 16),
                    const SizedBox(width: 8),
                    const Text('Scan & save'),
                  ],
                ),
              ),
            ),
          ],
        ),

        const SizedBox(height: 8),

        // Backfill covers button.
        SizedBox(
          width: double.infinity,
          child: OutlinedButton.icon(
            onPressed: backfillDisabled ? null : onBackfillCovers,
            style: outlinedStyle,
            icon: Icon(PhosphorIcons.imageSquare(), size: 16),
            label: const Text('Backfill album covers'),
          ),
        ),
      ],
    );
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// Status Section
// ═══════════════════════════════════════════════════════════════════════════

class _StatusSection extends StatelessWidget {
  const _StatusSection({
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
    final brightness = Theme.of(context).brightness;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Scanned count.
        _StatusRow(
          icon: PhosphorIcons.musicNotes(),
          label: 'Preview results',
          value: '$scannedCount songs',
        ),

        const SizedBox(height: 10),

        // Save status.
        if (lastSave != null) ...[
          _StatusRow(
            icon: PhosphorIcons.checkCircle(),
            label: 'Last save',
            value:
                'Scanned ${lastSave!.scanned}, saved ${lastSave!.saved}, skipped ${lastSave!.skipped}',
          ),
          const SizedBox(height: 10),
        ],

        // Watcher status.
        Row(
          children: [
            Icon(
              watchRunning ? PhosphorIcons.eye() : PhosphorIcons.eyeSlash(),
              size: 14,
              color: watchRunning
                  ? FlatColors.secondary(brightness)
                  : FlatColors.textSecondary(brightness),
            ),
            const SizedBox(width: 6),
            Text(
              'File watcher',
              style: FlatTypography.bodySmall(brightness),
            ),
            const Spacer(),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
              decoration: BoxDecoration(
                color: (watchRunning
                        ? FlatColors.secondary(brightness)
                        : FlatColors.textSecondary(brightness))
                    .withValues(alpha: 0.16),
                borderRadius: BorderRadius.circular(999),
              ),
              child: Text(
                watchRunning ? 'Running' : 'Stopped',
                style: FlatTypography.caption(brightness).copyWith(
                  fontSize: 10,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ],
        ),

        // Watcher events.
        if (watchEventCount > 0) ...[
          const SizedBox(height: 8),
          _StatusRow(
            icon: PhosphorIcons.arrowCounterClockwise(),
            label: 'Events',
            value:
                '$watchEventCount total, $watchPendingEvents pending',
          ),
        ],

        // Last event.
        if (lastWatchEventPath != null && lastWatchEventPath!.isNotEmpty) ...[
          const SizedBox(height: 8),
          _StatusRow(
            icon: PhosphorIcons.file(),
            label: 'Last event',
            value: '${lastWatchEventKind ?? 'unknown'} - $lastWatchEventPath',
            maxLines: 1,
          ),
        ],

        // Error.
        if (error != null) ...[
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: FlatColors.error(brightness).withValues(alpha: 0.1),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(
                color: FlatColors.error(brightness).withValues(alpha: 0.3),
                width: 0.5,
              ),
            ),
            child: Row(
              children: [
                Icon(
                  PhosphorIcons.warningCircle(),
                  size: 16,
                  color: FlatColors.error(brightness),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    error!,
                    style: FlatTypography.bodySmall(brightness).copyWith(
                      color: FlatColors.error(brightness),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// Preview Songs Section
// ═══════════════════════════════════════════════════════════════════════════

class _PreviewSongsSection extends StatelessWidget {
  const _PreviewSongsSection(this.results);

  final List<ScannedSong> results;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final displayCount = results.length > 20 ? 20 : results.length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // Song list.
        for (var i = 0; i < displayCount; i++)
          _SongTile(
            song: results[i],
            index: i,
          ),

        // More indicator.
        if (results.length > displayCount)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Center(
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                decoration: BoxDecoration(
                  color: FlatColors.textSecondary(brightness)
                      .withValues(alpha: 0.16),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  '+${results.length - displayCount} more',
                  style: FlatTypography.caption(brightness).copyWith(
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}

// ═══════════════════════════════════════════════════════════════════════════
// Shared Components
// ═══════════════════════════════════════════════════════════════════════════

/// Directory path chip.
class _DirectoryChip extends StatelessWidget {
  const _DirectoryChip({
    required this.path,
    required this.onRemove,
  });

  final String path;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final isDark = brightness == Brightness.dark;

    // Extract folder name from path.
    final folderName = path.split(RegExp(r'[/\\]')).last;
    final parentPath = path.substring(0, path.length - folderName.length);

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: onRemove,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          decoration: BoxDecoration(
            color: isDark
                ? Colors.white.withValues(alpha: 0.05)
                : Colors.black.withValues(alpha: 0.03),
            borderRadius: BorderRadius.circular(10),
            border: Border.all(
              color: isDark
                  ? Colors.white.withValues(alpha: 0.08)
                  : Colors.black.withValues(alpha: 0.08),
              width: 0.5,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              // Folder icon.
              Icon(
                PhosphorIcons.folder(),
                size: 16,
                color: FlatColors.primary(brightness),
              ),
              const SizedBox(width: 8),

              // Path info.
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 220),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      folderName,
                      overflow: TextOverflow.ellipsis,
                      style: FlatTypography.bodySmall(brightness).copyWith(
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    if (parentPath.isNotEmpty)
                      Text(
                        parentPath,
                        overflow: TextOverflow.ellipsis,
                        style: FlatTypography.caption(brightness).copyWith(
                          color: FlatColors.textTertiary(brightness),
                          fontSize: 10,
                        ),
                      ),
                  ],
                ),
              ),
              const SizedBox(width: 8),

              // Remove button.
              Container(
                padding: const EdgeInsets.all(3),
                decoration: BoxDecoration(
                  color: const Color(0xFFEF4444).withValues(alpha: 0.14),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  PhosphorIcons.x(),
                  size: 10,
                  color: const Color(0xFFEF4444),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Empty state placeholder.
class _EmptyState extends StatelessWidget {
  const _EmptyState({
    required this.icon,
    required this.message,
  });

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return Container(
      padding: const EdgeInsets.symmetric(vertical: 24),
      child: Center(
        child: Column(
          children: [
            Icon(
              icon,
              size: 32,
              color: FlatColors.textTertiary(brightness),
            ),
            const SizedBox(height: 8),
            Text(
              message,
              style: FlatTypography.caption(brightness),
            ),
          ],
        ),
      ),
    );
  }
}

/// Status row with icon, label, and value.
class _StatusRow extends StatelessWidget {
  const _StatusRow({
    required this.icon,
    required this.label,
    required this.value,
    this.maxLines,
  });

  final IconData icon;
  final String label;
  final String value;
  final int? maxLines;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final tone = switch (label) {
      'Preview results' => const Color(0xFF3B82F6),
      'Last save' => const Color(0xFF10B981),
      'Events' => const Color(0xFFF59E0B),
      'Last event' => const Color(0xFF8B5CF6),
      _ => FlatColors.textSecondary(brightness),
    };

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(
          icon,
          size: 14,
          color: tone,
        ),
        const SizedBox(width: 6),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                label,
                style: FlatTypography.caption(brightness).copyWith(
                  color: FlatColors.textSecondary(brightness),
                ),
              ),
              Text(
                value,
                maxLines: maxLines,
                overflow: maxLines != null ? TextOverflow.ellipsis : null,
                style: FlatTypography.bodySmall(brightness).copyWith(
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Song preview tile.
class _SongTile extends StatelessWidget {
  const _SongTile({
    required this.song,
    required this.index,
  });

  final ScannedSong song;
  final int index;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: Colors.transparent,
        border: Border(
          bottom: BorderSide(
            color: MacosBorder.color(brightness),
            width: 0.5,
          ),
        ),
      ),
      child: Row(
        children: [
          // Track number.
          SizedBox(
            width: 24,
            child: Text(
              '${index + 1}',
              style: FlatTypography.caption(brightness),
              textAlign: TextAlign.center,
            ),
          ),
          const SizedBox(width: 12),

          // Music icon.
          Icon(
            PhosphorIcons.musicNote(),
            size: 16,
            color: FlatColors.primary(brightness),
          ),
          const SizedBox(width: 12),

          // Song info.
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  song.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: FlatTypography.bodySmall(brightness).copyWith(
                    fontWeight: FontWeight.w500,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  '${song.artist} — ${song.album}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: FlatTypography.caption(brightness),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Scan progress indicator with animation.
class _ScanProgressIndicator extends StatefulWidget {
  const _ScanProgressIndicator();

  @override
  State<_ScanProgressIndicator> createState() =>
      _ScanProgressIndicatorState();
}

class _ScanProgressIndicatorState extends State<_ScanProgressIndicator>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1500),
    )..repeat();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return BayinGlassGroup(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                RotationTransition(
                  turns: _controller,
                  child: Icon(
                    PhosphorIcons.arrowsClockwise(),
                    size: 16,
                    color: FlatColors.primary(brightness),
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  'Scanning...',
                  style: FlatTypography.bodySmall(brightness).copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: LinearProgressIndicator(
                backgroundColor: FlatColors.stateLayer(
                  brightness,
                  FlatStateIntensity.standard,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
