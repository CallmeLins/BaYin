import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class UpdateSoftwarePage extends ConsumerStatefulWidget {
  const UpdateSoftwarePage({super.key});

  @override
  ConsumerState<UpdateSoftwarePage> createState() => _UpdateSoftwarePageState();
}

class _UpdateSoftwarePageState extends ConsumerState<UpdateSoftwarePage> {

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
    final brightness = Theme.of(context).brightness;
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Update Software'),
          left: BayinGhostIconButton(
            icon: PhosphorIcons.caretLeft(),
            tooltip: 'Back',
            onTap: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xs + 2),
                    child: Row(
                      children: [
                        Text(
                          'SOFTWARE INFO',
                          style: FlatTypography.label(brightness),
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
                  ),
                  Container(
                    padding: const EdgeInsets.all(FlatSpacing.md),
                    decoration: BoxDecoration(
                      color: brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.04)
                          : Colors.black.withValues(alpha: 0.02),
                      borderRadius: BorderRadius.circular(16),
                      border: Border.all(
                        color: brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.08)
                            : Colors.black.withValues(alpha: 0.06),
                        width: 0.8,
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Current channel: stable',
                          style: FlatTypography.bodySmall(brightness),
                        ),
                        const SizedBox(height: FlatSpacing.sm),
                        Text(
                          'Current app version: $_versionLabel',
                          style: FlatTypography.bodySmall(brightness),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: FlatSpacing.lg),
              Row(
                children: [
                  FilledButton.icon(
                    onPressed: _checking ? null : _checkUpdates,
                    icon: _checking
                        ? SizedBox(
                            width: 14,
                            height: 14,
                            child: CircularProgressIndicator(
                              strokeWidth: 2,
                              color: FlatColors.onPrimary(brightness),
                            ),
                          )
                        : Icon(PhosphorIcons.downloadSimple(), size: 18),
                    label: Text(_checking ? 'Checking...' : 'Check for updates'),
                  ),
                ],
              ),
              if (_status != null) ...[
                const SizedBox(height: FlatSpacing.md),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: FlatSpacing.md),
                  child: Text(
                    _status!,
                    style: FlatTypography.caption(brightness),
                  ),
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
