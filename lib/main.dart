import 'dart:convert';

import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'src/i18n/strings.g.dart';
import 'src/providers/providers.dart';
import 'src/router/app_router.dart';
import 'src/rust/rust_api.dart';
import 'src/services/library_service.dart';
import 'src/services/media_session_service.dart';
import 'src/services/file_watcher_service.dart';
import 'src/services/scan_service.dart';
import 'src/services/settings_service.dart';
import 'src/services/window_service.dart';
import 'src/theme/app_theme.dart';

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
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      themeMode: themeMode,
      locale: locale ?? TranslationProvider.of(context).flutterLocale,
      supportedLocales: AppLocaleUtils.supportedLocales,
      localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
        GlobalWidgetsLocalizations.delegate,
      ],
      routerConfig: router,
      builder: (context, child) {
        return _ResponsiveSync(child: child ?? const SizedBox.shrink());
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
