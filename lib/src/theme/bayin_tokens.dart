import 'package:flutter/material.dart';

import 'design_tokens.dart';
import 'macos_design_tokens.dart';

/// Surface tokens for BaYin — supports both flat and glassmorphism modes.
///
/// Accessed via [bayinTokensProvider] (Riverpod).
///
/// These tokens represent semantic surface roles (window background, sidebar,
/// titlebar, player, etc.) with support for macOS-style glass materials.

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
    this.windowGlassMaterial = GlassMaterialType.thick,
    this.sidebarGlassMaterial = GlassMaterialType.thin,
    this.barGlassMaterial = GlassMaterialType.ultra,
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

  // ── Glass Material Properties ────────────────────────────────────────

  /// Glass material type for window background.
  final GlassMaterialType windowGlassMaterial;

  /// Glass material type for sidebar.
  final GlassMaterialType sidebarGlassMaterial;

  /// Glass material type for bars and popovers.
  final GlassMaterialType barGlassMaterial;

  /// Primary text color suitable for body text on the window surface.
  Color get textPrimary =>
      isDark ? FlatColors.foregroundDark : FlatColors.foregroundLight;

  /// Muted / secondary text color.
  Color get textSecondary => FlatColors.textSecondary(
        isDark ? Brightness.dark : Brightness.light,
      );
}

/// Light flat surface tokens — Spotify-flat three-tier mapping.
const lightTokens = BayinTokens(
  windowBg: FlatColors.surfaceLight,
  titlebarBg: FlatColors.surfaceContainerLight,
  sidebarBg: FlatColors.surfaceContainerLight,
  barBg: FlatColors.surfaceContainerHighLight,
  popoverBg: FlatColors.surfaceContainerHighLight,
  playerBg: Color(0xFF060607), // Always-dark player background
  separator: FlatColors.borderLight,
  separatorAlpha: 1.0,
  separatorSoftAlpha: 0.5,
  highlight: FlatColors.primaryLight,
  windowGlassMaterial: GlassMaterialType.thick,
  sidebarGlassMaterial: GlassMaterialType.thin,
  barGlassMaterial: GlassMaterialType.ultra,
);

/// Dark flat surface tokens — Spotify-flat three-tier mapping.
const darkTokens = BayinTokens(
  windowBg: FlatColors.surfaceDark,
  titlebarBg: FlatColors.surfaceContainerDark,
  sidebarBg: FlatColors.surfaceContainerDark,
  barBg: FlatColors.surfaceContainerHighDark,
  popoverBg: FlatColors.surfaceContainerHighDark,
  playerBg: Color(0xFF060607), // Always-dark player background
  separator: FlatColors.borderDark,
  separatorAlpha: 1.0,
  separatorSoftAlpha: 0.5,
  highlight: FlatColors.primaryDark,
  windowGlassMaterial: GlassMaterialType.thick,
  sidebarGlassMaterial: GlassMaterialType.thin,
  barGlassMaterial: GlassMaterialType.ultra,
);
