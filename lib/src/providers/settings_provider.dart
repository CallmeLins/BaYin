import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../i18n/strings.g.dart';
import '../services/settings_service.dart';

final settingsServiceProvider = Provider<SettingsService>((ref) {
  return SettingsService.instance;
});

ThemeMode _parseThemeMode(String? raw) {
  switch (raw) {
    case 'light':
      return ThemeMode.light;
    case 'dark':
      return ThemeMode.dark;
    case 'system':
    default:
      return ThemeMode.system;
  }
}

String _serializeThemeMode(ThemeMode mode) {
  switch (mode) {
    case ThemeMode.light:
      return 'light';
    case ThemeMode.dark:
      return 'dark';
    case ThemeMode.system:
      return 'system';
  }
}

class ThemeModeController extends Notifier<ThemeMode> {
  @override
  ThemeMode build() {
    final service = ref.watch(settingsServiceProvider);
    return _parseThemeMode(service.readString(SettingsService.keyThemeMode));
  }

  Future<void> setThemeMode(ThemeMode mode) async {
    state = mode;
    final service = ref.read(settingsServiceProvider);
    await service.writeString(
      SettingsService.keyThemeMode,
      _serializeThemeMode(mode),
    );
  }
}

final themeModeProvider =
    NotifierProvider<ThemeModeController, ThemeMode>(ThemeModeController.new);

class LocaleController extends Notifier<Locale?> {
  @override
  Locale? build() {
    final service = ref.watch(settingsServiceProvider);
    final raw = service.readString(SettingsService.keyLocale);
    if (raw == null || raw.isEmpty) return null;
    final parts = raw.split('-');
    if (parts.isEmpty) return null;
    return Locale(parts.first, parts.length > 1 ? parts.sublist(1).join('_') : null);
  }

  Future<void> setLocale(Locale? locale) async {
    state = locale;
    final service = ref.read(settingsServiceProvider);
    if (locale == null) {
      await service.remove(SettingsService.keyLocale);
      LocaleSettings.useDeviceLocale();
      return;
    }
    final tag = locale.countryCode == null || locale.countryCode!.isEmpty
        ? locale.languageCode
        : '${locale.languageCode}-${locale.countryCode}';
    await service.writeString(SettingsService.keyLocale, tag);
    await LocaleSettings.setLocale(AppLocaleUtils.parse(tag));
  }
}

final localeProvider =
    NotifierProvider<LocaleController, Locale?>(LocaleController.new);
