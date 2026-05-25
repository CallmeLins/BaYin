import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../widgets/widgets.dart';

class UpdateSoftwarePage extends ConsumerStatefulWidget {
  const UpdateSoftwarePage({super.key});

  @override
  ConsumerState<UpdateSoftwarePage> createState() => _UpdateSoftwarePageState();
}

class _UpdateSoftwarePageState extends ConsumerState<UpdateSoftwarePage> {
  static final Uri _releasePage = Uri.parse('https://github.com/CallmeLins/BaYin/releases/latest');

  bool _checking = false;
  String _status = 'idle';
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
              const SizedBox(height: 48),

              // ── App Icon ────────────────────────────────────────────
              Center(
                child: Container(
                  width: 96,
                  height: 96,
                  decoration: BoxDecoration(
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.10)
                        : Colors.black.withValues(alpha: 0.05),
                    borderRadius: BorderRadius.circular(22),
                  ),
                  alignment: Alignment.center,
                  child: Icon(
                    PhosphorIcons.arrowCircleUp(),
                    size: 40,
                    color: const Color(0xFF3B82F6),
                  ),
                ),
              ),
              const SizedBox(height: 24),

              // ── App Name ───────────────────────────────────────────
              Center(
                child: Text(
                  'BaYin',
                  style: TextStyle(
                    fontSize: 24,
                    fontWeight: FontWeight.w700,
                    color: brightness == Brightness.dark
                        ? Colors.white
                        : Colors.black,
                  ),
                ),
              ),
              const SizedBox(height: 8),

              // ── Version ────────────────────────────────────────────
              Center(
                child: Text(
                  'Version $_versionLabel',
                  style: TextStyle(
                    fontSize: 14,
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.55)
                        : Colors.black.withValues(alpha: 0.50),
                  ),
                ),
              ),
              const SizedBox(height: 32),

              // ── Check Button ───────────────────────────────────────
              Center(
                child: FilledButton.icon(
                  onPressed: _checking ? null : _checkUpdates,
                  style: FilledButton.styleFrom(
                    padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(14),
                    ),
                    elevation: 0,
                  ),
                  icon: _checking
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: Colors.white,
                          ),
                        )
                      : Icon(PhosphorIcons.arrowClockwise(), size: 16),
                  label: Text(
                    _checking ? 'Checking...' : 'Check for Updates',
                    style: const TextStyle(
                      fontSize: 15,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 24),

              // ── Status Card ────────────────────────────────────────
              BayinGlassGroup(
                child: Padding(
                  padding: const EdgeInsets.all(16),
                  child: Text(
                    _statusText(),
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 14,
                      color: _status == 'error'
                          ? const Color(0xFFEF4444)
                          : (brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.70)
                              : Colors.black.withValues(alpha: 0.65)),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  String _statusText() {
    switch (_status) {
      case 'idle':
        return 'Tap the button above to check for updates.';
      case 'checking':
        return 'Checking for updates...';
      case 'upToDate':
        return 'You are running the latest version.';
      case 'updateAvailable':
        return 'A new version is available. Opening releases page...';
      case 'error':
        return 'Unable to check for updates. Please visit the releases page manually.';
      default:
        return '';
    }
  }

  Future<void> _checkUpdates() async {
    setState(() {
      _checking = true;
      _status = 'checking';
    });

    final opened = await launchUrl(
      _releasePage,
      mode: LaunchMode.externalApplication,
    );

    if (!mounted) return;
    setState(() {
      _checking = false;
      _status = opened ? 'updateAvailable' : 'error';
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
