import 'package:flutter/material.dart';

/// Glass surface / separator tokens ported from
/// `src-ui/src/index.css` (the `--bayin-*` custom properties).
///
/// Exposed via `Theme.of(context).extension<BayinTokens>()`.
@immutable
class BayinTokens extends ThemeExtension<BayinTokens> {
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

  @override
  BayinTokens copyWith({
    Color? windowBg,
    Color? titlebarBg,
    Color? sidebarBg,
    Color? barBg,
    Color? popoverBg,
    Color? playerBg,
    Color? separator,
    double? separatorAlpha,
    double? separatorSoftAlpha,
    Color? highlight,
  }) {
    return BayinTokens(
      windowBg: windowBg ?? this.windowBg,
      titlebarBg: titlebarBg ?? this.titlebarBg,
      sidebarBg: sidebarBg ?? this.sidebarBg,
      barBg: barBg ?? this.barBg,
      popoverBg: popoverBg ?? this.popoverBg,
      playerBg: playerBg ?? this.playerBg,
      separator: separator ?? this.separator,
      separatorAlpha: separatorAlpha ?? this.separatorAlpha,
      separatorSoftAlpha: separatorSoftAlpha ?? this.separatorSoftAlpha,
      highlight: highlight ?? this.highlight,
    );
  }

  @override
  BayinTokens lerp(ThemeExtension<BayinTokens>? other, double t) {
    if (other is! BayinTokens) return this;
    return BayinTokens(
      windowBg: Color.lerp(windowBg, other.windowBg, t) ?? windowBg,
      titlebarBg: Color.lerp(titlebarBg, other.titlebarBg, t) ?? titlebarBg,
      sidebarBg: Color.lerp(sidebarBg, other.sidebarBg, t) ?? sidebarBg,
      barBg: Color.lerp(barBg, other.barBg, t) ?? barBg,
      popoverBg: Color.lerp(popoverBg, other.popoverBg, t) ?? popoverBg,
      playerBg: Color.lerp(playerBg, other.playerBg, t) ?? playerBg,
      separator: Color.lerp(separator, other.separator, t) ?? separator,
      separatorAlpha:
          _lerpDouble(separatorAlpha, other.separatorAlpha, t),
      separatorSoftAlpha:
          _lerpDouble(separatorSoftAlpha, other.separatorSoftAlpha, t),
      highlight: Color.lerp(highlight, other.highlight, t) ?? highlight,
    );
  }
}

double _lerpDouble(double a, double b, double t) => a + (b - a) * t;

/// Convert a CSS-style HSL triple (h in degrees, s/l in 0–100) to a Flutter
/// Color. Matches the `hsl(var(--token))` expansion done by shadcn + Tailwind.
Color hslColor(double h, double s, double l) {
  return HSLColor.fromAHSL(1.0, h, s / 100.0, l / 100.0).toColor();
}
