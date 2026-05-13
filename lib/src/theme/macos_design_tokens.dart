import 'package:flutter/material.dart';

/// Glass material types for macOS-style frosted glass effects.
///
/// Each type has different blur radius and saturation values:
/// - [thin]: Sidebar/underlay. Blur=24, Saturation=1.5.
/// - [thick]: Main window content. Blur=40, Saturation=1.8.
/// - [ultra]: Popovers, menus. Blur=32, Saturation=1.6.
enum GlassMaterialType {
  /// Thin material: Sidebar, underlay.
  thin,

  /// Thick material: Main window content.
  thick,

  /// Ultra material: Popovers, menus.
  ultra,
}

/// macOS Sequoia Design System — Glassmorphism Tokens
///
/// Translates Apple's Human Interface Guidelines into Flutter.
/// Core principles: Physical glass materials, Retina borders, layered depth.
abstract final class MacosDesignTokens {
  MacosDesignTokens._();

  // ═══════════════════════════════════════════════════════════════════════════
  // GLASS MATERIALS (Vibrancy 3.0)
  // ═══════════════════════════════════════════════════════════════════════════

  /// Blur radii for different material types.
  static const double blurThin = 24.0;
  static const double blurThick = 40.0;
  static const double blurUltra = 32.0;

  /// Saturation multiplier for glass materials.
  static const double saturationThin = 1.5;
  static const double saturationThick = 1.8;
  static const double saturationUltra = 1.6;

  // ── Light Mode Glass Colors ──────────────────────────────────────────

  static const Color glassThinLight = Color(0x99F2F2F7);
  static const Color glassThickLight = Color(0xCCFFFFFF);
  static const Color glassUltraLight = Color(0xE6FFFFFF);

  // ── Dark Mode Glass Colors ───────────────────────────────────────────

  static const Color glassThinDark = Color(0x991E1E1E);
  static const Color glassThickDark = Color(0xB3282828);
  static const Color glassUltraDark = Color(0xE6323232);

  // ═══════════════════════════════════════════════════════════════════════════
  // RETINA BORDERS (The 0.5px Rule)
  // ═══════════════════════════════════════════════════════════════════════════

  static const double hairlineWidth = 0.5;
  static const Color borderLight = Color(0x0D000000);
  static const Color borderDark = Color(0x1AFFFFFF);
  static const Color bezelHighlightLight = Color(0x66FFFFFF);
  static const Color bezelHighlightDark = Color(0x33FFFFFF);

  // ═══════════════════════════════════════════════════════════════════════════
  // TRAFFIC LIGHTS
  // ═══════════════════════════════════════════════════════════════════════════

  static const double trafficLightSize = 12.0;
  static const double trafficLightSpacing = 8.0;
  static const Color trafficLightClose = Color(0xFFFF5F57);
  static const Color trafficLightMinimize = Color(0xFFFFBD2E);
  static const Color trafficLightMaximize = Color(0xFF28C840);
  static const Color trafficLightInactive = Color(0xFFCCCCCC);

  // ═══════════════════════════════════════════════════════════════════════════
  // ANIMATION (Apple Spring)
  // ═══════════════════════════════════════════════════════════════════════════

  static const Duration durationMicro = Duration(milliseconds: 150);
  static const Duration durationStandard = Duration(milliseconds: 200);
  static const Duration durationLarge = Duration(milliseconds: 300);
  static const double buttonTapScale = 0.96;

  // ═══════════════════════════════════════════════════════════════════════════
  // NOISE TEXTURE
  // ═══════════════════════════════════════════════════════════════════════════

  static const double noiseOpacity = 0.015;
}

/// Brightness-aware accessor for macOS glass materials.
class MacosGlass {
  const MacosGlass._();

  static Color thin(Brightness b) => b == Brightness.light
      ? MacosDesignTokens.glassThinLight
      : MacosDesignTokens.glassThinDark;

  static Color thick(Brightness b) => b == Brightness.light
      ? MacosDesignTokens.glassThickLight
      : MacosDesignTokens.glassThickDark;

  static Color ultra(Brightness b) => b == Brightness.light
      ? MacosDesignTokens.glassUltraLight
      : MacosDesignTokens.glassUltraDark;

  static double get thinBlur => MacosDesignTokens.blurThin;
  static double get thickBlur => MacosDesignTokens.blurThick;
  static double get ultraBlur => MacosDesignTokens.blurUltra;
}

/// Brightness-aware accessor for Retina borders.
class MacosBorder {
  const MacosBorder._();

  static Color color(Brightness b) => b == Brightness.light
      ? MacosDesignTokens.borderLight
      : MacosDesignTokens.borderDark;

  static Color bezelHighlight(Brightness b) => b == Brightness.light
      ? MacosDesignTokens.bezelHighlightLight
      : MacosDesignTokens.bezelHighlightDark;
}

/// Brightness-aware accessor for shadows.
class MacosShadow {
  const MacosShadow._();

  static List<BoxShadow> window(Brightness b) => b == Brightness.light
      ? [
          const BoxShadow(
            color: Color(0x66000000),
            blurRadius: 1,
            spreadRadius: 0,
            offset: Offset.zero,
          ),
          const BoxShadow(
            color: Color(0x33000000),
            blurRadius: 36,
            spreadRadius: 0,
            offset: Offset(0, 16),
          ),
        ]
      : [
          const BoxShadow(
            color: Color(0x80000000),
            blurRadius: 1,
            spreadRadius: 0,
            offset: Offset.zero,
          ),
          const BoxShadow(
            color: Color(0x4D000000),
            blurRadius: 48,
            spreadRadius: 0,
            offset: Offset(0, 20),
          ),
        ];

  static List<BoxShadow> card(Brightness b) => b == Brightness.light
      ? [
          const BoxShadow(
            color: Color(0x1A000000),
            blurRadius: 1,
            spreadRadius: 0,
            offset: Offset.zero,
          ),
          const BoxShadow(
            color: Color(0x0D000000),
            blurRadius: 12,
            spreadRadius: 0,
            offset: Offset(0, 4),
          ),
        ]
      : [
          const BoxShadow(
            color: Color(0x33000000),
            blurRadius: 1,
            spreadRadius: 0,
            offset: Offset.zero,
          ),
          const BoxShadow(
            color: Color(0x1A000000),
            blurRadius: 16,
            spreadRadius: 0,
            offset: Offset(0, 6),
          ),
        ];
}
