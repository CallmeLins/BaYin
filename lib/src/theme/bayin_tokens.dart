import 'package:flutter/foundation.dart';
import 'package:flutter/painting.dart';

/// Glass surface / separator tokens ported from
/// src-ui/src/index.css (the --bayin- custom properties).
///
/// Accessed via [bayinTokensProvider] (Riverpod), not ThemeExtension.
@immutable
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

  final Color windowBg;
  final Color titlebarBg;
  final Color sidebarBg;
  final Color barBg;
  final Color popoverBg;
  final Color playerBg;
  final Color separator;
  final double separatorAlpha;
  final double separatorSoftAlpha;
  final Color highlight;

  Color get separatorColor => separator.withValues(alpha: separatorAlpha);
  Color get separatorSoftColor =>
      separator.withValues(alpha: separatorSoftAlpha);

  /// True when the background luminance suggests a dark surface.
  bool get isDark => windowBg.computeLuminance() < 0.5;

  /// A text color suitable for body text on this surface.
  Color get textPrimary =>
      isDark ? const Color(0xFFF3F3F3) : const Color(0xFF0A0A0A);

  /// A muted text color for secondary content.
  Color get textSecondary =>
      isDark ? const Color(0x99FFFFFF) : const Color(0x99000000);
}

/// Convert a CSS-style HSL triple (h in degrees, s/l in 0–100) to a Flutter
/// Color. Matches the hsl(var(--token)) expansion done by shadcn + Tailwind.
Color hslColor(double h, double s, double l) {
  return HSLColor.fromAHSL(1.0, h, s / 100.0, l / 100.0).toColor();
}
