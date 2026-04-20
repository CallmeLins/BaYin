import 'dart:async';

import 'package:flutter/material.dart';

class UpdateSoftwarePage extends StatefulWidget {
  const UpdateSoftwarePage({super.key});

  @override
  State<UpdateSoftwarePage> createState() => _UpdateSoftwarePageState();
}

class _UpdateSoftwarePageState extends State<UpdateSoftwarePage> {
  bool _checking = false;
  String? _status;

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
      children: [
        Text(
          'Update Software',
          style: Theme.of(context).textTheme.titleLarge,
        ),
        const SizedBox(height: 12),
        Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHigh,
            borderRadius: BorderRadius.circular(12),
          ),
          child: const Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Current channel: stable'),
              SizedBox(height: 6),
              Text('Current app version: 0.1.0'),
            ],
          ),
        ),
        const SizedBox(height: 10),
        FilledButton.icon(
          onPressed: _checking ? null : _checkUpdates,
          icon: _checking
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: Colors.white,
                  ),
                )
              : const Icon(Icons.system_update),
          label: Text(_checking ? 'Checking...' : 'Check for updates'),
        ),
        if (_status != null) ...[
          const SizedBox(height: 10),
          Text(
            _status!,
            style: TextStyle(color: Theme.of(context).colorScheme.onSurfaceVariant),
          ),
        ],
      ],
    );
  }

  Future<void> _checkUpdates() async {
    setState(() {
      _checking = true;
      _status = null;
    });
    await Future<void>.delayed(const Duration(seconds: 1));
    if (!mounted) return;
    setState(() {
      _checking = false;
      _status = 'No updates available right now.';
    });
  }
}
