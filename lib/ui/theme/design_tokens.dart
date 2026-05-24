import 'package:flutter/material.dart';

/// Flat Design System — centralized token definitions for BaYin.
///
/// Philosophy: Zero artificial depth. Hierarchy through size, color, typography.
/// No shadows, no gradients on interactive elements, no blur. Color blocks and
/// geometric shapes define structure.
///
/// All widgets MUST reference these tokens rather than hardcoding values.

// =============================================================================
// Color Palette
// =============================================================================

/// Flat design color palette. Light and dark variants for each semantic role.
/// WCAG AA verified: foreground on background, onPrimary on primary, etc.
abstract final class FlatColors {
  FlatColors._();

  // ── Light palette ──────────────────────────────────────────────────────

  static const Color backgroundLight = Color(0xFFFFFFFF);
  static const Color foregroundLight = Color(0xFF111827);
  static const Color primaryLight = Color(0xFF3B82F6);
  static const Color primaryHoverLight = Color(0xFF2563EB);
  static const Color onPrimaryLight = Color(0xFFFFFFFF);
  static const Color secondaryLight = Color(0xFF10B981);
  static const Color secondaryHoverLight = Color(0xFF059669);
  static const Color onSecondaryLight = Color(0xFFFFFFFF);
  static const Color accentLight = Color(0xFFF59E0B);
  static const Color accentHoverLight = Color(0xFFD97706);
  static const Color onAccentLight = Color(0xFF111827);
  static const Color mutedLight = Color(0xFFF3F4F6);
  static const Color mutedHoverLight = Color(0xFFE5E7EB);
  static const Color borderLight = Color(0xFFE5E7EB);
  static const Color errorLight = Color(0xFFEF4444);
  static const Color onErrorLight = Color(0xFFFFFFFF);

  // ── Dark palette ───────────────────────────────────────────────────────

  // backgroundDark slightly raised from 0xFF0A0B0F to give surfaceContainer/-High
  // tiers room to step lighter without losing depth on OLED displays.
  static const Color backgroundDark = Color(0xFF0F1014);
  static const Color foregroundDark = Color(0xFFF3F4F6);
  static const Color primaryDark = Color(0xFF60A5FA);
  static const Color primaryHoverDark = Color(0xFF3B82F6);
  static const Color onPrimaryDark = Color(0xFF0F172A);
  static const Color secondaryDark = Color(0xFF34D399);
  static const Color secondaryHoverDark = Color(0xFF10B981);
  static const Color onSecondaryDark = Color(0xFF0F172A);
  static const Color accentDark = Color(0xFFFBBF24);
  static const Color accentHoverDark = Color(0xFFF59E0B);
  static const Color onAccentDark = Color(0xFF111827);
  static const Color mutedDark = Color(0xFF1A1D24);
  static const Color mutedHoverDark = Color(0xFF2D3139);
  static const Color borderDark = Color(0xFF2D3139);
  static const Color errorDark = Color(0xFFF87171);
  static const Color onErrorDark = Color(0xFF0A0B0F);

  // ── Surface tier (Spotify-flat) — three solid tones for hierarchy ───────
  //
  // surface              = window background (deepest)
  // surfaceContainer     = sidebar / titlebar / chrome (one tier up)
  // surfaceContainerHigh = player bar / cards / popover / input fill (two up)
  //
  // Hierarchy through tone, not shadow/blur. This is the Spotify-flat core.

  static const Color surfaceLight = backgroundLight;
  static const Color surfaceContainerLight = Color(0xFFF4F5F7);
  static const Color surfaceContainerHighLight = Color(0xFFECEDEF);

  static const Color surfaceDark = backgroundDark;
  static const Color surfaceContainerDark = Color(0xFF181A1F);
  static const Color surfaceContainerHighDark = Color(0xFF22252B);

  // ── Brightness-aware accessors ─────────────────────────────────────────

  static Color background(Brightness b) =>
      b == Brightness.light ? backgroundLight : backgroundDark;
  static Color foreground(Brightness b) =>
      b == Brightness.light ? foregroundLight : foregroundDark;
  static Color primary(Brightness b) =>
      b == Brightness.light ? primaryLight : primaryDark;
  static Color primaryHover(Brightness b) =>
      b == Brightness.light ? primaryHoverLight : primaryHoverDark;
  static Color onPrimary(Brightness b) =>
      b == Brightness.light ? onPrimaryLight : onPrimaryDark;
  static Color secondary(Brightness b) =>
      b == Brightness.light ? secondaryLight : secondaryDark;
  static Color secondaryHover(Brightness b) =>
      b == Brightness.light ? secondaryHoverLight : secondaryHoverDark;
  static Color onSecondary(Brightness b) =>
      b == Brightness.light ? onSecondaryLight : onSecondaryDark;
  static Color accent(Brightness b) =>
      b == Brightness.light ? accentLight : accentDark;
  static Color accentHover(Brightness b) =>
      b == Brightness.light ? accentHoverLight : accentHoverDark;
  static Color onAccent(Brightness b) =>
      b == Brightness.light ? onAccentLight : onAccentDark;
  static Color muted(Brightness b) =>
      b == Brightness.light ? mutedLight : mutedDark;
  static Color mutedHover(Brightness b) =>
      b == Brightness.light ? mutedHoverLight : mutedHoverDark;
  static Color border(Brightness b) =>
      b == Brightness.light ? borderLight : borderDark;
  static Color error(Brightness b) =>
      b == Brightness.light ? errorLight : errorDark;
  static Color onError(Brightness b) =>
      b == Brightness.light ? onErrorLight : onErrorDark;

  // ── Surface tier accessors ────────────────────────────────────────────

  static Color surface(Brightness b) =>
      b == Brightness.light ? surfaceLight : surfaceDark;
  static Color surfaceContainer(Brightness b) =>
      b == Brightness.light ? surfaceContainerLight : surfaceContainerDark;
  static Color surfaceContainerHigh(Brightness b) => b == Brightness.light
      ? surfaceContainerHighLight
      : surfaceContainerHighDark;

  /// Unified state-layer overlay for hover / active / pressed states.
  /// Use over any surface to express interaction without bespoke alpha values.
  static Color stateLayer(Brightness b, FlatStateIntensity intensity) {
    final base = b == Brightness.light
        ? const Color(0xFF000000)
        : const Color(0xFFFFFFFF);
    final alpha = switch (intensity) {
      FlatStateIntensity.subtle => 0.04,
      FlatStateIntensity.standard => 0.08,
      FlatStateIntensity.emphasized => 0.12,
      FlatStateIntensity.pressed => 0.16,
    };
    return base.withValues(alpha: alpha);
  }

  /// Text secondary (muted) color for a given brightness.
  static Color textSecondary(Brightness b) =>
      b == Brightness.light
          ? const Color(0x99000000) // 60% black
          : const Color(0x99FFFFFF); // 60% white

  /// Text tertiary (very muted) for captions and placeholders.
  static Color textTertiary(Brightness b) =>
      b == Brightness.light
          ? const Color(0x66000000) // 40% black
          : const Color(0x66FFFFFF); // 40% white
}

// =============================================================================
// Typography
// =============================================================================

/// Flat design typography scale.
///
/// Preferred font: Outfit (geometric sans-serif). Falls back to system geometric
/// sans-serifs: SF Pro Display (Apple), Inter (Android/Web).
abstract final class FlatTypography {
  FlatTypography._();

  static const String _fontFamily = 'SF Pro Text';

  /// Bold headings with tight letter-spacing for geometric precision.
  /// Size: 32sp, Weight: ExtraBold (800), Letter-spacing: -0.02em
  static TextStyle heading1(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 32,
        fontWeight: FontWeight.w800,
        letterSpacing: -0.02 * 32,
        height: 1.15,
        color: FlatColors.foreground(brightness),
      );

  /// Size: 24sp, Weight: Bold (700), Letter-spacing: -0.015em
  static TextStyle heading2(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 24,
        fontWeight: FontWeight.w700,
        letterSpacing: -0.015 * 24,
        height: 1.2,
        color: FlatColors.foreground(brightness),
      );

  /// Size: 20sp, Weight: SemiBold (600), Letter-spacing: -0.01em
  static TextStyle heading3(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 20,
        fontWeight: FontWeight.w600,
        letterSpacing: -0.01 * 20,
        height: 1.25,
        color: FlatColors.foreground(brightness),
      );

  /// Compact heading — 16sp, weight 700, tight tracking.
  /// For inline page headers and section titles where heading3 (20sp) is too
  /// large but body text feels too quiet.
  static TextStyle headingCompact(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 16,
        fontWeight: FontWeight.w700,
        letterSpacing: -0.01 * 16,
        height: 1.25,
        color: FlatColors.foreground(brightness),
      );

  /// Size: 16sp, Weight: Regular (400), Line-height: 1.5
  static TextStyle body(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 16,
        fontWeight: FontWeight.w400,
        height: 1.5,
        color: FlatColors.foreground(brightness),
      );

  /// Size: 14sp, Weight: Medium (500)
  static TextStyle bodySmall(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 14,
        fontWeight: FontWeight.w500,
        height: 1.45,
        color: FlatColors.foreground(brightness),
      );

  /// Size: 14sp, Weight: SemiBold (600), used for buttons.
  static TextStyle button(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 14,
        fontWeight: FontWeight.w600,
        letterSpacing: 0.02 * 14,
        height: 1.2,
        color: FlatColors.foreground(brightness),
      );

  /// Size: 12sp, Weight: SemiBold (600), Uppercase, used for section labels.
  static TextStyle label(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 11,
        fontWeight: FontWeight.w600,
        letterSpacing: 0.05 * 11,
        height: 1.3,
        color: FlatColors.textSecondary(brightness),
      );

  /// Size: 12sp, Weight: Regular (400), used for captions and metadata.
  static TextStyle caption(Brightness brightness) => TextStyle(
        fontFamily: _fontFamily,
        fontSize: 12,
        fontWeight: FontWeight.w400,
        height: 1.35,
        color: FlatColors.textSecondary(brightness),
      );
}

// =============================================================================
// Spacing
// =============================================================================

/// Spacing scale in multiples of 4dp.
abstract final class FlatSpacing {
  FlatSpacing._();

  static const double xs = 4;
  static const double sm = 8;

  /// 12dp — between sm (8) and md (16). Use for row/list-item internal gaps
  /// instead of `sm + 4` patches.
  static const double smPlus = 12;

  static const double md = 16;

  /// 20dp — between md (16) and lg (24). Use for card padding / section gaps.
  static const double mdPlus = 20;

  static const double lg = 24;
  static const double xl = 32;
  static const double xxl = 48;
  static const double xxxl = 64;
  static const double huge = 96;

  /// Page horizontal padding for different breakpoints.
  static double pageH(Breakpoint breakpoint) => switch (breakpoint) {
        Breakpoint.compact => 16,
        Breakpoint.medium => 24,
        Breakpoint.wide => 32,
        Breakpoint.car => 48,
      };

  /// Card internal padding.
  static double cardPadding(Breakpoint breakpoint) => switch (breakpoint) {
        Breakpoint.compact => 16,
        Breakpoint.medium => 20,
        Breakpoint.wide => 24,
        Breakpoint.car => 32,
      };

  /// Minimum touch target size (ensures accessibility).
  static double touchTarget(Breakpoint breakpoint) => switch (breakpoint) {
        Breakpoint.compact => 48,
        Breakpoint.medium => 48,
        Breakpoint.wide => 56,
        Breakpoint.car => 64,
      };
}

// =============================================================================
// Border Radius
// =============================================================================

/// Consistent border radius across the entire UI (6–8dp).
abstract final class FlatRadius {
  FlatRadius._();

  /// Small elements: tags, badges, small icons.
  static const double sm = 4;

  /// Default: buttons, inputs, cards, list tiles.
  static const double md = 8;

  /// Larger containers, dialogs, sheets.
  static const double lg = 12;

  /// Circular elements (avatars, icon circles).
  /// Use with BoxShape.circle for perfect circles.
  static const double circle = 999;
}

// =============================================================================
// Animation Durations
// =============================================================================

/// Snappy, digital-native motion. No bouncy springs, no slow fades.
abstract final class FlatDurations {
  FlatDurations._();

  /// Micro-interactions: hover feedback, icon color shifts.
  static const Duration micro = Duration(milliseconds: 150);

  /// Standard transitions: button hover scale, card hover, toggle switches.
  static const Duration standard = Duration(milliseconds: 200);

  /// Larger transitions: section reveals, page transitions, sheet popups.
  static const Duration large = Duration(milliseconds: 300);

  /// Standard ease curve for all transitions.
  static const Curve curve = Curves.easeOut;
}

// =============================================================================
// Breakpoints
// =============================================================================

/// Responsive breakpoints for viewport size categories.
enum Breakpoint {
  /// Phone: <600dp shortest side.
  compact,

  /// Tablet: 600–900dp.
  medium,

  /// Desktop: >900dp.
  wide,

  /// Car infotainment / large touch displays.
  car,
}

/// Compute breakpoint from a given viewport size.
Breakpoint computeBreakpoint(Size size) {
  final w = size.shortestSide;
  if (w < 600) return Breakpoint.compact;
  if (w < 900) return Breakpoint.medium;
  return Breakpoint.wide;
}

/// Whether the current breakpoint is a wide (desktop-class) layout.
bool isWideBreakpoint(Breakpoint bp) =>
    bp == Breakpoint.wide || bp == Breakpoint.car;

// =============================================================================
// Border Width (Structural)
// =============================================================================

/// Thick, intentional borders only. No 0.5px hairlines.
abstract final class FlatBorder {
  FlatBorder._();

  /// Structural dividers (FAQ, section splits). Bold and intentional.
  static const double structural = 2.0;

  /// Input focus ring, active indicators. High contrast.
  static const double focus = 2.0;

  /// Outline buttons, selected cards.
  static const double emphasis = 4.0;
}

/// Intensity tiers for [FlatColors.stateLayer]. Maps to alpha 4/8/12/16%.
enum FlatStateIntensity {
  /// 4% — hover on quiet/secondary surfaces.
  subtle,

  /// 8% — hover on chrome / standard interactive surfaces.
  standard,

  /// 12% — active state, selected indicators.
  emphasized,

  /// 16% — pressed / dragging.
  pressed,
}

// =============================================================================
// Platform-adaptive helpers
// =============================================================================

/// Returns a scaled value appropriate for the target platform + breakpoint.
/// Car mode gets 1.4x scaling for touch safety.
double platformScale({
  required Breakpoint breakpoint,
  required double compact,
  required double medium,
  required double wide,
}) {
  final base = switch (breakpoint) {
    Breakpoint.compact => compact,
    Breakpoint.medium => medium,
    Breakpoint.wide => wide,
    Breakpoint.car => wide * 1.4,
  };
  if (breakpoint == Breakpoint.car) return base;
  return base;
}
