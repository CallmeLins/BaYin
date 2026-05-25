import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../services/scan_service.dart';
import '../../utils/info_bar_helper.dart';
import '../../widgets/widgets.dart';
import 'folder_browser.dart';

class ScanMusicPage extends ConsumerStatefulWidget {
  const ScanMusicPage({super.key});

  @override
  ConsumerState<ScanMusicPage> createState() => _ScanMusicPageState();
}

class _ScanMusicPageState extends ConsumerState<ScanMusicPage> {
  final TextEditingController _pathController = TextEditingController();
  List<String> _directories = <String>[];
  bool _skipShort = false;
  bool _configApplied = false;

  @override
  void initState() {
    super.initState();
  }

  @override
  void dispose() {
    _pathController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scanner = ref.watch(scannerProvider);
    final config = ref.watch(scanConfigProvider);
    _applyConfigOnce(config);

    final isScanning = scanner.isLoading;

    return Column(
      children: [
        const BayinPageHeader(
          title: Text('Scan Music'),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 16),

              // ── Sources Section ───────────────────────────────────────
              Text(
                'Sources',
                style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                  letterSpacing: -0.3,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                'Add local folders or connect a stream server to scan music into your library.',
                style: TextStyle(
                  fontSize: 14,
                  color: Theme.of(context).brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.55)
                      : Colors.black.withValues(alpha: 0.45),
                ),
              ),
              const SizedBox(height: 16),

              // ── Add Source Buttons ────────────────────────────────────
              Row(
                children: [
                  Expanded(
                    child: _AddSourceButton(
                      icon: PhosphorIcons.folderSimple(),
                      label: 'Add local source',
                      onTap: _browseFolder,
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: _AddSourceButton(
                      icon: PhosphorIcons.cloud(),
                      label: 'Add stream source',
                      onTap: () => context.go('/stream-config'),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),

              // ── Source List ───────────────────────────────────────────
              if (_directories.isEmpty)
                _EmptySourcesPlaceholder()
              else
                _SourceList(
                  directories: _directories,
                  onRemove: _removeDirectory,
                ),

              const SizedBox(height: 24),

              // ── Skip Short Audio ─────────────────────────────────────
              _OptionCard(
                title: 'Skip short audio',
                subtitle: 'Minimum duration 60 seconds',
                value: _skipShort,
                onChanged: (v) => setState(() => _skipShort = v),
              ),

              const SizedBox(height: 24),

              // ── Scan Button ──────────────────────────────────────────
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: isScanning || _directories.isEmpty
                      ? null
                      : _scanAndSave,
                  style: BayinButtonStyles.filledFlat().copyWith(
                    padding: WidgetStateProperty.all(
                      const EdgeInsets.symmetric(vertical: 16),
                    ),
                    shape: WidgetStateProperty.all(
                      RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(16),
                      ),
                    ),
                  ),
                  child: Text(
                    isScanning ? 'Scanning...' : 'Start Scan',
                    style: const TextStyle(
                      fontSize: 15,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ),

              // ── Scan Status ──────────────────────────────────────────
              if (scanner.results.isNotEmpty) ...[
                const SizedBox(height: 24),
                _ScanResultCard(
                  scannedCount: scanner.results.length,
                  lastSave: scanner.lastSave,
                  error: scanner.error,
                ),
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

  Future<void> _scanAndSave() async {
    try {
      await ref.read(scanServiceProvider).saveScanConfig(
            ScanConfig(
              directories: List<String>.unmodifiable(_directories),
              skipShort: _skipShort,
              minDuration: _skipShort ? 60 : 0,
              lastScanAt: DateTime.now().millisecondsSinceEpoch,
            ),
          );
      ref.invalidate(scanConfigProvider);
      if (!mounted) return;

      await ref.read(scannerProvider.notifier).scanAndSave(
            _directories,
            skipShortAudio: _skipShort,
            minDuration: _skipShort ? 60 : 0,
          );

      if (!mounted) return;
      showInfoMessage(context, 'Scan complete.');
    } catch (error) {
      if (!mounted) return;
      showInfoMessage(context, 'Scan failed: $error', isError: true);
    }
  }
}

// ── Add Source Button ────────────────────────────────────────────────────

class _AddSourceButton extends StatefulWidget {
  const _AddSourceButton({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  State<_AddSourceButton> createState() => _AddSourceButtonState();
}

class _AddSourceButtonState extends State<_AddSourceButton> {
  bool _pressing = false;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final bgColor = brightness == Brightness.dark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.04);

    return GestureDetector(
      onTap: widget.onTap,
      onTapDown: (_) => setState(() => _pressing = true),
      onTapUp: (_) => setState(() => _pressing = false),
      onTapCancel: () => setState(() => _pressing = false),
      child: AnimatedScale(
        duration: const Duration(milliseconds: 120),
        scale: _pressing ? 0.97 : 1.0,
        child: Container(
          padding: const EdgeInsets.symmetric(vertical: 14),
          decoration: BoxDecoration(
            color: bgColor,
            borderRadius: BorderRadius.circular(16),
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(widget.icon, size: 16),
              const SizedBox(width: 8),
              Text(
                widget.label,
                style: const TextStyle(
                  fontSize: 14,
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

// ── Empty Sources Placeholder ────────────────────────────────────────────

class _EmptySourcesPlaceholder extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(vertical: 40, horizontal: 20),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.12)
              : Colors.black.withValues(alpha: 0.10),
          width: 1,
          strokeAlign: BorderSide.strokeAlignInside,
        ),
      ),
      child: Text(
        'No sources configured yet.',
        textAlign: TextAlign.center,
        style: TextStyle(
          fontSize: 14,
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.45)
              : Colors.black.withValues(alpha: 0.40),
        ),
      ),
    );
  }
}

// ── Source List ──────────────────────────────────────────────────────────

class _SourceList extends StatelessWidget {
  const _SourceList({
    required this.directories,
    required this.onRemove,
  });

  final List<String> directories;
  final ValueChanged<String> onRemove;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.06)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.8,
        ),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child: Column(
          children: [
            for (var i = 0; i < directories.length; i++) ...[
              _SourceRow(
                path: directories[i],
                onRemove: () => onRemove(directories[i]),
              ),
              if (i < directories.length - 1)
                Divider(
                  height: 1,
                  thickness: 1,
                  indent: 64,
                  color: brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.06)
                      : Colors.black.withValues(alpha: 0.06),
                ),
            ],
          ],
        ),
      ),
    );
  }
}

// ── Source Row ───────────────────────────────────────────────────────────

class _SourceRow extends StatefulWidget {
  const _SourceRow({
    required this.path,
    required this.onRemove,
  });

  final String path;
  final VoidCallback onRemove;

  @override
  State<_SourceRow> createState() => _SourceRowState();
}

class _SourceRowState extends State<_SourceRow> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final folderName = widget.path.split(RegExp(r'[/\\]')).last;

    return MouseRegion(
      onEnter: (_) => setState(() => _hovering = true),
      onExit: (_) => setState(() => _hovering = false),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        color: _hovering
            ? (brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.04)
                : Colors.black.withValues(alpha: 0.03))
            : Colors.transparent,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        child: Row(
          children: [
            Container(
              width: 38,
              height: 38,
              decoration: BoxDecoration(
                color: const Color(0xFF3B82F6).withValues(
                  alpha: brightness == Brightness.dark ? 0.22 : 0.12,
                ),
                borderRadius: BorderRadius.circular(10),
                border: Border.all(
                  color: const Color(0xFF3B82F6).withValues(
                    alpha: brightness == Brightness.dark ? 0.38 : 0.22,
                  ),
                  width: 0.8,
                ),
              ),
              alignment: Alignment.center,
              child: Icon(
                PhosphorIcons.folderSimple(),
                size: 18,
                color: const Color(0xFF3B82F6),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    folderName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  Text(
                    widget.path,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 12,
                      color: brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.45)
                          : Colors.black.withValues(alpha: 0.40),
                    ),
                  ),
                ],
              ),
            ),
            GestureDetector(
              onTap: widget.onRemove,
              child: Container(
                padding: const EdgeInsets.all(6),
                decoration: BoxDecoration(
                  color: const Color(0xFFEF4444).withValues(alpha: 0.12),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  PhosphorIcons.x(),
                  size: 14,
                  color: const Color(0xFFEF4444),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Option Card ──────────────────────────────────────────────────────────

class _OptionCard extends StatelessWidget {
  const _OptionCard({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });

  final String title;
  final String subtitle;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.06)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.8,
        ),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: TextStyle(
                    fontSize: 12,
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.45)
                        : Colors.black.withValues(alpha: 0.40),
                  ),
                ),
              ],
            ),
          ),
          BayinToggleSwitch(
            value: value,
            onChanged: onChanged,
          ),
        ],
      ),
    );
  }
}

// ── Scan Result Card ─────────────────────────────────────────────────────

class _ScanResultCard extends StatelessWidget {
  const _ScanResultCard({
    required this.scannedCount,
    this.lastSave,
    this.error,
  });

  final int scannedCount;
  final ScanAndSaveResult? lastSave;
  final String? error;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.06)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.8,
        ),
      ),
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _ResultRow(
            icon: PhosphorIcons.musicNotes(),
            label: 'Scanned',
            value: '$scannedCount songs',
            color: const Color(0xFF3B82F6),
          ),
          if (lastSave != null) ...[
            const SizedBox(height: 10),
            _ResultRow(
              icon: PhosphorIcons.checkCircle(),
              label: 'Saved',
              value: '${lastSave!.saved}, skipped ${lastSave!.skipped}',
              color: const Color(0xFF10B981),
            ),
          ],
          if (error != null) ...[
            const SizedBox(height: 10),
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: const Color(0xFFEF4444).withValues(alpha: 0.10),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: [
                  Icon(
                    PhosphorIcons.warningCircle(),
                    size: 14,
                    color: const Color(0xFFEF4444),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      error!,
                      style: const TextStyle(
                        fontSize: 12,
                        color: Color(0xFFEF4444),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _ResultRow extends StatelessWidget {
  const _ResultRow({
    required this.icon,
    required this.label,
    required this.value,
    required this.color,
  });

  final IconData icon;
  final String label;
  final String value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Row(
      children: [
        Icon(icon, size: 14, color: color),
        const SizedBox(width: 8),
        Text(
          label,
          style: TextStyle(
            fontSize: 12,
            color: brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.55)
                : Colors.black.withValues(alpha: 0.45),
          ),
        ),
        const Spacer(),
        Text(
          value,
          style: const TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.w500,
          ),
        ),
      ],
    );
  }
}
