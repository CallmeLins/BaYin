import 'package:shared_preferences/shared_preferences.dart';

/// Tiny shared_preferences wrapper. Keeps key names centralised so consumers
/// never hit a stale/duplicated string.
class SettingsService {
  SettingsService._(this._prefs);

  static SettingsService? _instance;
  static SettingsService get instance {
    final inst = _instance;
    if (inst == null) {
      throw StateError(
        'SettingsService not initialised. Call SettingsService.init() first.',
      );
    }
    return inst;
  }

  static Future<SettingsService> init() async {
    final existing = _instance;
    if (existing != null) return existing;
    final prefs = await SharedPreferences.getInstance();
    final service = SettingsService._(prefs);
    _instance = service;
    return service;
  }

  final SharedPreferences _prefs;

  static const String keyThemeMode = 'settings.themeMode';
  static const String keyLocale = 'settings.locale';

  String? readString(String key) => _prefs.getString(key);
  Future<void> writeString(String key, String value) =>
      _prefs.setString(key, value);
  Future<void> remove(String key) => _prefs.remove(key);
}
