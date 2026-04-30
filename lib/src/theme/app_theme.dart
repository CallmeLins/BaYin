import 'package:fluent_ui/fluent_ui.dart';

/// Light / dark FluentThemeData for BaYin.
///
/// BayinTokens are exposed via [bayinTokensProvider] (Riverpod).
class AppTheme {
  const AppTheme._();

  static FluentThemeData light() => _buildTheme(Brightness.light);
  static FluentThemeData dark() => _buildTheme(Brightness.dark);

  static FluentThemeData _buildTheme(Brightness brightness) {
    final isLight = brightness == Brightness.light;
    return FluentThemeData(
      brightness: brightness,
      accentColor: isLight ? _lightAccent : _darkAccent,
      scaffoldBackgroundColor: isLight
          ? const Color(0xFFFFFFFF)
          : const Color(0xFF050508),
      activeColor: const Color(0xFF3B82F6),
      visualDensity: VisualDensity.standard,
    );
  }
}

final _lightAccent = AccentColor('normal', <String, Color>{
  'normal': const Color(0xFF3B82F6),
  'darkest': const Color(0xFF1E3A5F),
  'darker': const Color(0xFF1D4ED8),
  'dark': const Color(0xFF2563EB),
  'light': const Color(0xFFBFDBFE),
  'lighter': const Color(0xFFDBEAFE),
  'lightest': const Color(0xFFEFF6FF),
});

final _darkAccent = AccentColor('normal', <String, Color>{
  'normal': const Color(0xFF60A5FA),
  'darkest': const Color(0xFFDBEAFE),
  'darker': const Color(0xFFBFDBFE),
  'dark': const Color(0xFF93C5FD),
  'light': const Color(0xFF1E3A5F),
  'lighter': const Color(0xFF172554),
  'lightest': const Color(0xFF0F172A),
});
