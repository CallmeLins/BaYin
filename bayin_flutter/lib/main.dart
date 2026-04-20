import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'src/i18n/strings.g.dart';
import 'src/providers/providers.dart';
import 'src/router/app_router.dart';
import 'src/rust/rust_api.dart';
import 'src/services/library_service.dart';
import 'src/services/media_session_service.dart';
import 'src/services/settings_service.dart';
import 'src/services/window_service.dart';
import 'src/theme/app_theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await SettingsService.init();
  await WindowService.init();
  await _restoreLocale();
  await RustApi.instance.ensureInitialized();
  await LibraryService.instance.ensureDatabaseInitialized();
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

class BaYinApp extends ConsumerWidget {
  const BaYinApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(appRouterProvider);
    final themeMode = ref.watch(themeModeProvider);
    final locale = ref.watch(localeProvider);

    return MaterialApp.router(
      title: 'BaYin',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
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
        // Keep the responsiveLayoutProvider in sync with the live MediaQuery
        // so Riverpod consumers don't need to touch MediaQuery directly.
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
    // Schedule after build; updating a Notifier during build would trigger
    // the "setState during build" assertion.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(responsiveLayoutProvider.notifier).update(size);
    });
    return child;
  }
}
