import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as mat show showModalBottomSheet;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../rust/rust_api.dart';

class FolderBrowser extends ConsumerStatefulWidget {
  const FolderBrowser({
    super.key,
    required this.initialPath,
  });

  final String initialPath;

  static Future<String?> show(
    BuildContext context, {
    required String initialPath,
  }) {
    return mat.showModalBottomSheet<String>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      barrierColor: Colors.black.withValues(alpha: 0.42),
      showDragHandle: false,
      builder: (_) => FractionallySizedBox(
        heightFactor: 0.85,
        child: FolderBrowser(initialPath: initialPath),
      ),
    );
  }

  @override
  ConsumerState<FolderBrowser> createState() => _FolderBrowserState();
}

class _FolderBrowserState extends ConsumerState<FolderBrowser> {
  late String _currentPath;
  List<RustDirectoryEntry> _entries = const <RustDirectoryEntry>[];
  bool _isLoading = false;
  String? _error;
  final List<String> _history = <String>[];

  @override
  void initState() {
    super.initState();
    _currentPath = widget.initialPath;
    _load(_currentPath);
  }

  void _navigateTo(String path) {
    _history.add(_currentPath);
    _load(path);
  }

  void _goBack() {
    if (_history.isNotEmpty) {
      final previous = _history.removeLast();
      _load(previous, addToHistory: false);
    }
  }

  void _goToParent() {
    final directory = Directory(_currentPath);
    final parent = directory.parent.path;
    if (parent != _currentPath) {
      _navigateTo(parent);
    }
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final canGoBack = _history.isNotEmpty;

    return SafeArea(
      child: Container(
        decoration: BoxDecoration(
          color: brightness == Brightness.dark
              ? const Color(0xFF1C1C1E).withValues(alpha: 0.96)
              : Colors.white.withValues(alpha: 0.96),
          borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
          border: Border.all(
            color: brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.10)
                : Colors.black.withValues(alpha: 0.08),
            width: 0.8,
          ),
        ),
        child: Column(
          children: [
            // ── Header ─────────────────────────────────────────────────
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              child: Row(
                children: [
                  IconButton(
                    onPressed: canGoBack ? _goBack : null,
                    icon: Icon(PhosphorIcons.caretLeft()),
                    style: IconButton.styleFrom(
                      foregroundColor: canGoBack
                          ? (brightness == Brightness.dark
                              ? Colors.white
                              : Colors.black)
                          : (brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.25)
                              : Colors.black.withValues(alpha: 0.25)),
                    ),
                  ),
                  IconButton(
                    onPressed: _goToParent,
                    icon: Icon(PhosphorIcons.house()),
                  ),
                  const Spacer(),
                  Text(
                    'Select Folder',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const Spacer(),
                  IconButton(
                    onPressed: () => Navigator.of(context).pop(),
                    icon: Icon(PhosphorIcons.x()),
                  ),
                ],
              ),
            ),

            Divider(
              height: 1,
              thickness: 1,
              color: brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.06)
                  : Colors.black.withValues(alpha: 0.06),
            ),

            // ── Current Path ───────────────────────────────────────────
            Container(
              width: double.infinity,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              color: brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.04)
                  : Colors.black.withValues(alpha: 0.03),
              child: Text(
                _currentPath,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  fontSize: 14,
                  color: brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.55)
                      : Colors.black.withValues(alpha: 0.50),
                ),
              ),
            ),

            Divider(
              height: 1,
              thickness: 1,
              color: brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.06)
                  : Colors.black.withValues(alpha: 0.06),
            ),

            // ── Directory List ─────────────────────────────────────────
            if (_isLoading)
              const Expanded(
                child: Center(child: CircularProgressIndicator()),
              )
            else if (_error != null)
              Expanded(
                child: Center(
                  child: Padding(
                    padding: const EdgeInsets.all(24),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(
                          PhosphorIcons.warningCircle(),
                          size: 32,
                          color: const Color(0xFFEF4444),
                        ),
                        const SizedBox(height: 8),
                        Text(
                          _error!,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                            fontSize: 14,
                            color: Color(0xFFEF4444),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              )
            else if (_entries.isEmpty)
              Expanded(
                child: Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        PhosphorIcons.folderSimple(),
                        size: 48,
                        color: brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.20)
                            : Colors.black.withValues(alpha: 0.15),
                      ),
                      const SizedBox(height: 8),
                      Text(
                        'No subfolders',
                        style: TextStyle(
                          fontSize: 14,
                          color: brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.45)
                              : Colors.black.withValues(alpha: 0.40),
                        ),
                      ),
                    ],
                  ),
                ),
              )
            else
              Expanded(
                child: ListView.builder(
                  itemCount: _entries.length,
                  itemBuilder: (context, index) {
                    final entry = _entries[index];
                    return _FolderRow(
                      name: entry.name,
                      onTap: () => _navigateTo(entry.path),
                    );
                  },
                ),
              ),

            // ── Select Button ──────────────────────────────────────────
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
              child: SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: () => Navigator.of(context).pop(_currentPath),
                  style: FilledButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 14),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(14),
                    ),
                    elevation: 0,
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Icon(PhosphorIcons.check(), size: 18),
                      const SizedBox(width: 8),
                      const Text(
                        'Select This Folder',
                        style: TextStyle(
                          fontSize: 15,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _load(String path, {bool addToHistory = true}) async {
    if (path.isEmpty) return;
    setState(() {
      _isLoading = true;
      _error = null;
    });
    try {
      final entries = RustApi.instance.listDirectories(path);
      if (!mounted) return;
      setState(() {
        _currentPath = path;
        _entries = entries.where((item) => item.isDir).toList(growable: false);
        _isLoading = false;
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _error = 'Failed to load: $error';
      });
    }
  }
}

// ── Folder Row ───────────────────────────────────────────────────────────

class _FolderRow extends StatefulWidget {
  const _FolderRow({
    required this.name,
    required this.onTap,
  });

  final String name;
  final VoidCallback onTap;

  @override
  State<_FolderRow> createState() => _FolderRowState();
}

class _FolderRowState extends State<_FolderRow> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return MouseRegion(
      onEnter: (_) => setState(() => _hovering = true),
      onExit: (_) => setState(() => _hovering = false),
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: widget.onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          color: _hovering
              ? (brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.06)
                  : Colors.black.withValues(alpha: 0.04))
              : Colors.transparent,
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
          child: Row(
            children: [
              Icon(
                PhosphorIcons.folderSimple(),
                size: 20,
                color: const Color(0xFF3B82F6),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  widget.name,
                  style: const TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w400,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              Icon(
                PhosphorIcons.caretRight(),
                size: 16,
                color: brightness == Brightness.dark
                    ? Colors.white.withValues(alpha: 0.35)
                    : Colors.black.withValues(alpha: 0.30),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
