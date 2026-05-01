import 'package:flutter/material.dart';

import 'design_tokens.dart';

/// Flat surface tokens for BaYin — defines the color of each surface region.
///
/// Replaces the previous glass/blur surface tokens with flat, solid colors.
/// Accessed via [bayinTokensProvider] (Riverpod).
///
/// These tokens are derived from [FlatColors] and represent semantic surface
/// roles (window background, sidebar, titlebar, player, etc.).

class BayinTokens {
  const BayinTokens({
    required this.windowBg,
    required this.titlebarBg,
    required this.sidebarBg,
    required this.barBg,
    required this.popoverBg,
    required this.playerBg,
    required this.separator,
    required this.separatorAlpha,
    required this.separatorSoftAlpha,
    required this.highlight,
  });

  /// Main window / body background.
  final Color windowBg;

  /// Titlebar background (slightly distinct from window).
  final Color titlebarBg;

  /// Sidebar navigation background.
  final Color sidebarBg;

  /// Bottom bar / player bar background.
  final Color barBg;

  /// Popover / dropdown / overlay background.
  final Color popoverBg;

  /// Full-screen player background.
  final Color playerBg;

  /// Base separator color (before alpha blending).
  final Color separator;

  /// Alpha for standard separators (0.0–1.0).
  final double separatorAlpha;

  /// Alpha for soft/subtle separators (0.0–1.0).
  final double separatorSoftAlpha;

  /// Highlight / accent indicator color.
  final Color highlight;

  /// Blended separator color for standard use.
  Color get separatorColor => separator.withValues(alpha: separatorAlpha);

  /// Blended separator color for subtle use.
  Color get separatorSoftColor =>
      separator.withValues(alpha: separatorSoftAlpha);

  /// True when the window background is a dark surface.
  bool get isDark => windowBg.computeLuminance() < 0.5;

  /// Primary text color suitable for body text on the window surface.
  Color get textPrimary =>
      isDark ? FlatColors.foregroundDark : FlatColors.foregroundLight;

  /// Muted / secondary text color.
  Color get textSecondary => FlatColors.textSecondary(
        isDark ? Brightness.dark : Brightness.light,
      );
}

/// Light flat surface tokens.
const lightTokens = BayinTokens(
  windowBg: FlatColors.backgroundLight,
  titlebarBg: FlatColors.mutedLight,
  sidebarBg: FlatColors.surfaceAltLight,
  barBg: FlatColors.backgroundLight,
  popoverBg: FlatColors.backgroundLight,
  playerBg: Color(0xFF060607), // Always-dark player background
  separator: FlatColors.borderLight,
  separatorAlpha: 1.0,
  separatorSoftAlpha: 0.5,
  highlight: FlatColors.primaryLight,
);

/// Dark flat surface tokens.
const darkTokens = BayinTokens(
  windowBg: FlatColors.backgroundDark,
  titlebarBg: FlatColors.mutedDark,
  sidebarBg: FlatColors.surfaceAltDark,
  barBg: FlatColors.backgroundDark,
  popoverBg: FlatColors.mutedDark,
  playerBg: Color(0xFF060607), // Always-dark player background
  separator: FlatColors.borderDark,
  separatorAlpha: 1.0,
  separatorSoftAlpha: 0.5,
  highlight: FlatColors.primaryDark,
);

/// Convert CSS-style HSL to Flutter Color. Kept for backward compatibility
/// with any code still using the old CSS token approach.
Color hslColor(double h, double s, double l) {
  return HSLColor.fromAHSL(1.0, h, s / 100.0, l / 100.0).toColor();
}
