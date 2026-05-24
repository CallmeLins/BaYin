import 'dart:convert';

import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'ui/i18n/strings.g.dart';
import 'ui/providers/providers.dart';
import 'ui/router/app_router.dart';
import 'ui/rust/rust_api.dart';
import 'ui/services/library_service.dart';
import 'ui/services/media_session_service.dart';
import 'ui/services/file_watcher_service.dart';
import 'ui/services/scan_service.dart';
import 'ui/services/settings_service.dart';
import 'ui/services/window_service.dart';
import 'ui/theme/app_theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await SettingsService.init();
  await WindowService.init();
  await _restoreLocale();
  await RustApi.instance.ensureInitialized();
  _restoreAudioEngineSettings();
  await LibraryService.instance.ensureDatabaseInitialized();
  await _restoreFileWatcher();
  try {
    await MediaSessionService.instance.init();
  } catch (_) {
    // Audio session may be unavailable in some environments.
  }
  runApp(TranslationProvider(child: const ProviderScope(child: BaYinApp())));
}

Future<void> _restoreLocale() async {
  final stored = SettingsService.instance.readString(SettingsService.keyLocale);
  if (stored == null || stored.isEmpty) {
    LocaleSettings.useDeviceLocale();
    return;
  }
  final locale = AppLocaleUtils.parse(stored);
  await LocaleSettings.setLocale(locale);
}

void _restoreAudioEngineSettings() {
  final service = SettingsService.instance;
  final eqEnabled = service.readBool(SettingsService.keyEqEnabled) ?? false;
  final rawGains = service.readString(SettingsService.keyEqGains);
  var gains = List<double>.filled(10, 0.0);
  if (rawGains != null && rawGains.isNotEmpty) {
    try {
      final decoded = jsonDecode(rawGains);
      if (decoded is List<dynamic> && decoded.length == 10) {
        gains = decoded
            .map(
              (value) => (value as num).toDouble().clamp(-12.0, 12.0).toDouble(),
            )
            .toList(growable: false);
      }
    } catch (_) {
      gains = List<double>.filled(10, 0.0);
    }
  }

  try {
    RustApi.instance.audioSetEqEnabled(eqEnabled);
    RustApi.instance.audioSetEqGains(gains);
  } catch (_) {
    // If audio engine is unavailable during startup, keep app boot resilient.
  }
}

Future<void> _restoreFileWatcher() async {
  try {
    final config = await ScanService.instance.loadScanConfig();
    final directories = config?.directories ?? const <String>[];
    if (directories.isNotEmpty) {
      await FileWatcherService.instance.start(directories);
    }
  } catch (_) {
    // Watcher is optional and should not block startup.
  }
}

class BaYinApp extends ConsumerWidget {
  const BaYinApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(appRouterProvider);
    final themeMode = ref.watch(themeModeProvider);
    final locale = ref.watch(localeProvider);

    return FluentApp.router(
      title: 'BaYin',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.fluentLight(),
      darkTheme: AppTheme.fluentDark(),
      themeMode: themeMode,
      locale: locale ?? TranslationProvider.of(context).flutterLocale,
      supportedLocales: AppLocaleUtils.supportedLocales,
      localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      routerConfig: router,
      builder: (context, child) {
        // Wrap with a Material widget + flat Material ThemeData so that
        // Material widgets (TextField, FilledButton, Switch, ListTile,
        // Divider, etc.) have a proper ancestor and pick up flat styling.
        final brightness = FluentTheme.of(context).brightness;
        final theme = AppTheme.materialTheme(brightness);
        return Theme(
          data: theme,
          child: Material(
            type: MaterialType.transparency,
            textStyle: theme.textTheme.bodyMedium,
            child: _ResponsiveSync(child: child ?? const SizedBox.shrink()),
          ),
        );
      },
    );
  }
}

class _ResponsiveSync extends ConsumerWidget {
  const _ResponsiveSync({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final size = MediaQuery.sizeOf(context);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(responsiveLayoutProvider.notifier).update(size);
    });
    return child;
  }
}
