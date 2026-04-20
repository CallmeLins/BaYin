import 'package:flutter/material.dart';

import 'bayin_tokens.dart';

/// Light / dark ThemeData plus the ported BayinTokens (see src-ui/src/index.css).
///
/// Shadcn-style tokens map onto `ColorScheme` where natural; glass-material
/// tokens live on `BayinTokens` (a ThemeExtension). Consumers read them via
/// `Theme.of(context).colorScheme.xxx` or
/// `Theme.of(context).extension<BayinTokens>()!.xxx`.
class AppTheme {
  const AppTheme._();

  static ThemeData light() => _buildTheme(_lightScheme, _lightTokens, Brightness.light);
  static ThemeData dark() => _buildTheme(_darkScheme, _darkTokens, Brightness.dark);

  static ThemeData _buildTheme(
    ColorScheme scheme,
    BayinTokens tokens,
    Brightness brightness,
  ) {
    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: tokens.windowBg,
      canvasColor: tokens.windowBg,
      dividerColor: tokens.separatorColor,
      dividerTheme: DividerThemeData(
        color: tokens.separatorColor,
        thickness: 0.5,
        space: 0,
      ),
      extensions: <ThemeExtension<dynamic>>[tokens],
    );
  }
}

// ── Shadcn-equivalent light scheme ──────────────────────────────────────────
// Values lifted verbatim from src-ui/src/index.css `:root, :host` block.
final ColorScheme _lightScheme = ColorScheme(
  brightness: Brightness.light,
  primary: hslColor(240, 5.9, 10),
  onPrimary: hslColor(0, 0, 98),
  primaryContainer: hslColor(240, 4.8, 95.9),
  onPrimaryContainer: hslColor(240, 5.9, 10),
  secondary: hslColor(240, 4.8, 95.9),
  onSecondary: hslColor(240, 5.9, 10),
  secondaryContainer: hslColor(240, 4.8, 95.9),
  onSecondaryContainer: hslColor(240, 5.9, 10),
  tertiary: hslColor(240, 4.8, 95.9),
  onTertiary: hslColor(240, 5.9, 10),
  tertiaryContainer: hslColor(240, 4.8, 95.9),
  onTertiaryContainer: hslColor(240, 5.9, 10),
  error: hslColor(0, 84.2, 60.2),
  onError: hslColor(0, 0, 98),
  errorContainer: hslColor(0, 84.2, 60.2),
  onErrorContainer: hslColor(0, 0, 98),
  surface: hslColor(0, 0, 100),
  onSurface: hslColor(240, 10, 3.9),
  surfaceContainerHighest: hslColor(240, 4.8, 95.9),
  onSurfaceVariant: hslColor(240, 3.8, 46.1),
  outline: hslColor(240, 5.9, 90),
  outlineVariant: hslColor(240, 5.9, 90),
  shadow: Colors.black,
  scrim: Colors.black,
  inverseSurface: hslColor(240, 10, 3.9),
  onInverseSurface: hslColor(0, 0, 98),
  inversePrimary: hslColor(240, 5.9, 10),
);

final ColorScheme _darkScheme = ColorScheme(
  brightness: Brightness.dark,
  primary: hslColor(0, 0, 98),
  onPrimary: hslColor(240, 5.9, 10),
  primaryContainer: hslColor(240, 3.7, 15.9),
  onPrimaryContainer: hslColor(0, 0, 98),
  secondary: hslColor(240, 3.7, 15.9),
  onSecondary: hslColor(0, 0, 98),
  secondaryContainer: hslColor(240, 3.7, 15.9),
  onSecondaryContainer: hslColor(0, 0, 98),
  tertiary: hslColor(240, 3.7, 15.9),
  onTertiary: hslColor(0, 0, 98),
  tertiaryContainer: hslColor(240, 3.7, 15.9),
  onTertiaryContainer: hslColor(0, 0, 98),
  error: hslColor(0, 62.8, 30.6),
  onError: hslColor(0, 0, 98),
  errorContainer: hslColor(0, 62.8, 30.6),
  onErrorContainer: hslColor(0, 0, 98),
  surface: hslColor(240, 10, 3.9),
  onSurface: hslColor(0, 0, 98),
  surfaceContainerHighest: hslColor(240, 3.7, 15.9),
  onSurfaceVariant: hslColor(240, 5, 64.9),
  outline: hslColor(240, 3.7, 15.9),
  outlineVariant: hslColor(240, 3.7, 15.9),
  shadow: Colors.black,
  scrim: Colors.black,
  inverseSurface: hslColor(0, 0, 98),
  onInverseSurface: hslColor(240, 10, 3.9),
  inversePrimary: hslColor(0, 0, 98),
);

// ── BaYin glass-material tokens ─────────────────────────────────────────────
final BayinTokens _lightTokens = BayinTokens(
  windowBg: hslColor(0, 0, 100),
  titlebarBg: hslColor(240, 4.8, 96),
  sidebarBg: hslColor(240, 4.8, 96),
  barBg: hslColor(0, 0, 100),
  popoverBg: hslColor(0, 0, 100),
  playerBg: hslColor(240, 6, 7),
  separator: hslColor(240, 5.9, 90),
  separatorAlpha: 0.35,
  separatorSoftAlpha: 0.28,
  highlight: hslColor(0, 0, 100),
);

final BayinTokens _darkTokens = BayinTokens(
  windowBg: hslColor(240, 6, 7),
  titlebarBg: hslColor(240, 6, 10),
  sidebarBg: hslColor(240, 6, 12),
  barBg: hslColor(240, 6, 16),
  popoverBg: hslColor(240, 6, 18),
  playerBg: hslColor(240, 6, 7),
  separator: hslColor(0, 0, 100),
  separatorAlpha: 0.12,
  separatorSoftAlpha: 0.10,
  highlight: hslColor(0, 0, 100),
);
