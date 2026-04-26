import 'dart:async';

import 'package:flutter/material.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:url_launcher/url_launcher.dart';

class UpdateSoftwarePage extends StatefulWidget {
  const UpdateSoftwarePage({super.key});

  @override
  State<UpdateSoftwarePage> createState() => _UpdateSoftwarePageState();
}

class _UpdateSoftwarePageState extends State<UpdateSoftwarePage> {
  static final Uri _releasePage = Uri.parse('https://bayin.app');

  bool _checking = false;
  String? _status;
  String _versionLabel = '--';

  @override
  void initState() {
    super.initState();
    unawaited(_loadPackageInfo());
  }

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
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text('Current channel: stable'),
              const SizedBox(height: 6),
              Text('Current app version: $_versionLabel'),
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

    final opened = await launchUrl(
      _releasePage,
      mode: LaunchMode.externalApplication,
    );

    if (!mounted) return;
    setState(() {
      _checking = false;
      _status = opened
          ? 'Opened release page in your browser.'
          : 'Unable to open release page. Please visit https://bayin.app manually.';
    });
  }

  Future<void> _loadPackageInfo() async {
    try {
      final info = await PackageInfo.fromPlatform();
      if (!mounted) return;
      setState(() {
        _versionLabel = '${info.version} (${info.buildNumber})';
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _versionLabel = 'Unknown';
      });
    }
  }
}
