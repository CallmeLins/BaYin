import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../widgets/widgets.dart';

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
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Update Software'),
          left: IconButton(
            tooltip: 'Back',
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
            icon: Icon(PhosphorIcons.caretLeft()),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 10),
              const SizedBox(height: 12),
        BayinGlassCard(
          padding: const EdgeInsets.all(12),
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
          ),
        ),
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
