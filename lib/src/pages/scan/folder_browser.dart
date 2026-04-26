import 'dart:io';

import 'package:flutter/material.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../rust/rust_api.dart';

class FolderBrowser extends StatefulWidget {
  const FolderBrowser({
    super.key,
    required this.initialPath,
  });

  final String initialPath;

  static Future<String?> show(
    BuildContext context, {
    required String initialPath,
  }) {
    return showModalBottomSheet<String>(
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
  State<FolderBrowser> createState() => _FolderBrowserState();
}

class _FolderBrowserState extends State<FolderBrowser> {
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
    return SafeArea(
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _pathController,
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                      isDense: true,
                      hintText: 'Folder path',
                    ),
                    onSubmitted: _load,
                  ),
                ),
                const SizedBox(width: 8),
                IconButton.filled(
                  onPressed: () => _load(_pathController.text.trim()),
                  icon: const Icon(Icons.arrow_forward),
                  tooltip: 'Go',
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
            child: Row(
              children: [
                OutlinedButton.icon(
                  onPressed: _goParent,
                  icon: Icon(PhosphorIcons.arrowUp()),
                  label: const Text('Up'),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    _currentPath,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
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
          if (_isLoading) const LinearProgressIndicator(minHeight: 2),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 12, 8),
              child: Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          Expanded(
            child: ListView.builder(
              itemCount: _entries.length,
              itemBuilder: (context, index) {
                final entry = _entries[index];
                return ListTile(
                  dense: true,
                  leading: Icon(PhosphorIcons.folder()),
                  title: Text(entry.name),
                  subtitle: Text(
                    entry.path,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Icon(PhosphorIcons.caretRight()),
                  onTap: () => _load(entry.path),
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
