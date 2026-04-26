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
  static const String keyShowCoverInList = 'settings.ui.showCoverInList';
  static const String keyVisualizerEnabled = 'settings.visualizer.enabled';
  static const String keyBassEffectEnabled = 'settings.visualizer.bassEffect';
  static const String keyProEnabled = 'settings.pro.enabled';
  static const String keyProGlassEnabled = 'settings.pro.glass';
  static const String keyProColorSpectrumEnabled = 'settings.pro.colorSpectrum';
  static const String keyProPureModeEnabled = 'settings.pro.pureModeFeature';
  static const String keyPureModeEnabled = 'settings.pro.pureModeActive';
  static const String keyLyricsFontSize = 'settings.lyrics.fontSize';
  static const String keyLyricsTranslationFontSize =
      'settings.lyrics.translationFontSize';
  static const String keyLyricsOffsetMs = 'settings.lyrics.offsetMs';
  static const String keyLyricsPosition = 'settings.lyrics.position';
  static const String keyLyricsSelectable = 'settings.lyrics.selectable';
  static const String keyLyricsWordByWordAnimation =
      'settings.lyrics.wordByWordAnimation';
  static const String keyLyricsAutoBlur = 'settings.lyrics.autoBlur';
  static const String keyEqEnabled = 'settings.eq.enabled';
  static const String keyEqPreset = 'settings.eq.preset';
  static const String keyEqGains = 'settings.eq.gains';

  String? readString(String key) => _prefs.getString(key);
  bool? readBool(String key) => _prefs.getBool(key);
  int? readInt(String key) => _prefs.getInt(key);
  double? readDouble(String key) => _prefs.getDouble(key);

  Future<void> writeString(String key, String value) =>
      _prefs.setString(key, value);
  Future<void> writeBool(String key, bool value) => _prefs.setBool(key, value);
  Future<void> writeInt(String key, int value) => _prefs.setInt(key, value);
  Future<void> writeDouble(String key, double value) =>
      _prefs.setDouble(key, value);

  Future<void> remove(String key) => _prefs.remove(key);
}
