import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter/material.dart' as mat show showModalBottomSheet;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
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
      showDragHandle: true,
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
  late final TextEditingController _pathController;
  late String _currentPath;
  List<RustDirectoryEntry> _entries = const <RustDirectoryEntry>[];
  bool _isLoading = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _currentPath = widget.initialPath;
    _pathController = TextEditingController(text: _currentPath);
    _load(_currentPath);
  }

  @override
  void dispose() {
    _pathController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final tokens = ref.watch(bayinTokensProvider);
    return SafeArea(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
            child: Row(
              children: [
                Expanded(
                  child: TextBox(
                    controller: _pathController,
                    placeholder: 'Folder path',
                    onSubmitted: (value) => _load(value),
                  ),
                ),
                const SizedBox(width: 8),
                FilledButton(
                  onPressed: () => _load(_pathController.text.trim()),
                  child: Icon(PhosphorIcons.arrowRight()),
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
            child: Row(
              children: [
                OutlinedButton(
                  onPressed: _goParent,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(PhosphorIcons.arrowUp()),
                      const SizedBox(width: 4),
                      const Text('Up'),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    _currentPath,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(color: tokens.textSecondary),
                  ),
                ),
                const SizedBox(width: 8),
                FilledButton(
                  onPressed: () => Navigator.of(context).pop(_currentPath),
                  child: const Text('Select'),
                ),
              ],
            ),
          ),
          if (_isLoading) const ProgressBar(),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
              child: Text(
                _error!,
                style: TextStyle(color: Colors.red),
              ),
            ),
          Expanded(
            child: ListView.builder(
              itemCount: _entries.length,
              itemBuilder: (context, index) {
                final entry = _entries[index];
                return GestureDetector(
                  onTap: () => _load(entry.path),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                    child: Row(
                      children: [
                        Icon(PhosphorIcons.folder()),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(entry.name),
                              Text(
                                entry.path,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(fontSize: 12),
                              ),
                            ],
                          ),
                        ),
                        Icon(PhosphorIcons.caretRight()),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _load(String path) async {
    if (path.isEmpty) {
      return;
    }
    setState(() {
      _isLoading = true;
      _error = null;
    });
    try {
      final entries = RustApi.instance.listDirectories(path);
      if (!mounted) {
        return;
      }
      setState(() {
        _currentPath = path;
        _pathController.text = path;
        _entries = entries.where((item) => item.isDir).toList(growable: false);
        _isLoading = false;
      });
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _isLoading = false;
        _error = '$error';
      });
    }
  }

  void _goParent() {
    final directory = Directory(_currentPath);
    final parent = directory.parent.path;
    if (parent == _currentPath) {
      return;
    }
    _load(parent);
  }
}
