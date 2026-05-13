import 'package:flutter/material.dart';
import 'package:fluent_ui/fluent_ui.dart' as ft;

import 'design_tokens.dart';
import 'macos_design_tokens.dart';

/// Flat Design System — Theme builders for BaYin.
///
/// Two theme layers:
/// 1. [FluentThemeData] — for the FluentApp shell and fluent_ui widgets
/// 2. [ThemeData] — for Material widgets, injected via a Theme wrapper in main.dart
///
/// Both layers are flattened: zero shadows, no surface tints, no gradients
/// on interactive elements. The design tokens in [FlatColors], [FlatTypography],
/// [FlatSpacing], and [FlatRadius] are the single source of truth for all
/// visual decisions.
final class AppTheme {
  const AppTheme._();

  // ── Fluent Theme (app shell) ─────────────────────────────────────────

  /// Build flat [ft.FluentThemeData] for light mode.
  static ft.FluentThemeData fluentLight() =>
      _buildFluent(Brightness.light);

  /// Build flat [ft.FluentThemeData] for dark mode.
  static ft.FluentThemeData fluentDark() =>
      _buildFluent(Brightness.dark);

  static ft.FluentThemeData _buildFluent(Brightness brightness) {
    final isLight = brightness == Brightness.light;
    final primary = FlatColors.primary(brightness);
    final bg = FlatColors.background(brightness);

    // Build a flat AccentColor from our primary palette.
    final accent = ft.AccentColor('normal', <String, Color>{
      'normal': primary,
      'darkest': isLight ? const Color(0xFF1E3A5F) : const Color(0xFFDBEAFE),
      'darker': FlatColors.primaryHover(brightness),
      'dark': FlatColors.primaryHover(brightness),
      'light': isLight ? const Color(0xFFBFDBFE) : const Color(0xFF1E3A5F),
      'lighter': isLight ? const Color(0xFFDBEAFE) : const Color(0xFF172554),
      'lightest': isLight ? const Color(0xFFEFF6FF) : const Color(0xFF0F172A),
    });

    return ft.FluentThemeData(
      brightness: brightness,
      accentColor: accent,
      scaffoldBackgroundColor: bg,
      activeColor: primary,
      visualDensity: VisualDensity.standard,
    );
  }

  // ── Material Theme (for Material widgets) ────────────────────────────

  /// Build flat Material [ThemeData]. Injected via Theme wrapper in main.dart
  /// so that Material widgets (FilledButton, SegmentedButton, Switch, etc.)
  /// pick up flat design styling.
  static ThemeData materialTheme(Brightness brightness) {
    final isLight = brightness == Brightness.light;
    final bg = FlatColors.background(brightness);
    final fg = FlatColors.foreground(brightness);
    final primary = FlatColors.primary(brightness);
    final onPrimary = FlatColors.onPrimary(brightness);
    final secondary = FlatColors.secondary(brightness);
    final muted = FlatColors.muted(brightness);
    final surfaceContainer = FlatColors.surfaceContainer(brightness);
    final surfaceContainerHigh = FlatColors.surfaceContainerHigh(brightness);
    final border = FlatColors.border(brightness);
    final error = FlatColors.error(brightness);

    // CRITICAL: Flat ColorScheme — no surface tint colors.
    final colorScheme = ColorScheme(
      brightness: brightness,
      primary: primary,
      onPrimary: onPrimary,
      primaryContainer: isLight
          ? const Color(0xFFDBEAFE)
          : const Color(0xFF1E3A5F),
      onPrimaryContainer: isLight
          ? const Color(0xFF1E3A5F)
          : const Color(0xFFDBEAFE),
      secondary: secondary,
      onSecondary: FlatColors.onSecondary(brightness),
      secondaryContainer: isLight
          ? const Color(0xFFD1FAE5)
          : const Color(0xFF064E3B),
      onSecondaryContainer: isLight
          ? const Color(0xFF064E3B)
          : const Color(0xFFD1FAE5),
      tertiary: FlatColors.accent(brightness),
      onTertiary: FlatColors.onAccent(brightness),
      error: error,
      onError: FlatColors.onError(brightness),
      surface: bg,
      onSurface: fg,
      surfaceContainerHighest: muted,
      onSurfaceVariant: FlatColors.textSecondary(brightness),
      outline: border,
      outlineVariant: border,
      shadow: Colors.transparent,
      surfaceTint: Colors.transparent,
    );

    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: colorScheme,
      scaffoldBackgroundColor: bg,

      // ── Typography ─────────────────────────────────────────────────
      textTheme: TextTheme(
        displayLarge: FlatTypography.heading1(brightness),
        displayMedium: FlatTypography.heading2(brightness),
        displaySmall: FlatTypography.heading3(brightness),
        bodyLarge: FlatTypography.body(brightness),
        bodyMedium: FlatTypography.bodySmall(brightness),
        bodySmall: FlatTypography.caption(brightness),
        labelLarge: FlatTypography.button(brightness),
        labelSmall: FlatTypography.label(brightness),
      ),

      // ── AppBar (flat) ──────────────────────────────────────────────
      appBarTheme: AppBarTheme(
        backgroundColor: surfaceContainer,
        foregroundColor: fg,
        elevation: 0,
        shadowColor: Colors.transparent,
        surfaceTintColor: Colors.transparent,
        scrolledUnderElevation: 0,
      ),

      // ── Buttons — macOS glass style ────────────────────────────────
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          // macOS gradient: subtle vertical gradient.
          backgroundColor: primary,
          foregroundColor: onPrimary,
          elevation: 0,
          shadowColor: Colors.transparent,
          surfaceTintColor: Colors.transparent,
          disabledBackgroundColor: primary.withValues(alpha: 0.5),
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(6), // macOS uses 6px
          ),
          // Inner top highlight for 3D bezel effect.
          side: BorderSide(
            color: Colors.white.withValues(alpha: 0.2),
            width: 0.5,
          ),
          textStyle: FlatTypography.button(brightness).copyWith(
            color: onPrimary,
          ),
        ),
      ),

      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: muted,
          foregroundColor: fg,
          elevation: 0,
          shadowColor: Colors.transparent,
          surfaceTintColor: Colors.transparent,
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(FlatRadius.md),
          ),
        ),
      ),

      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          backgroundColor: Colors.transparent,
          foregroundColor: primary,
          elevation: 0,
          shadowColor: Colors.transparent,
          surfaceTintColor: Colors.transparent,
          side: BorderSide(color: primary, width: FlatBorder.emphasis),
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(FlatRadius.md),
          ),
        ),
      ),

      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: primary,
          elevation: 0,
          shadowColor: Colors.transparent,
          surfaceTintColor: Colors.transparent,
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(FlatRadius.md),
          ),
        ),
      ),

      iconButtonTheme: IconButtonThemeData(
        style: IconButton.styleFrom(
          foregroundColor: fg,
          hoverColor: FlatColors.stateLayer(
            brightness,
            FlatStateIntensity.standard,
          ),
          minimumSize: const Size(48, 48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(FlatRadius.md),
          ),
        ),
      ),

      // ── Cards — macOS glass style ─────────────────────────────────
      cardTheme: CardThemeData(
        color: surfaceContainerHigh,
        // Subtle elevation for depth.
        elevation: 0,
        shadowColor: Colors.transparent,
        surfaceTintColor: Colors.transparent,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12), // macOS uses 12px for cards
          // Retina border: 0.5px hairline.
          side: BorderSide(
            color: MacosBorder.color(brightness),
            width: MacosDesignTokens.hairlineWidth,
          ),
        ),
      ),

      // ── Inputs — macOS glass style ────────────────────────────────
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: surfaceContainerHigh,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 12,
          vertical: 10,
        ),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(5), // macOS uses 5px
          borderSide: BorderSide.none,
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(5),
          borderSide: BorderSide(
            color: MacosBorder.color(brightness),
            width: MacosDesignTokens.hairlineWidth,
          ),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(5),
          // Glow ring on focus.
          borderSide: BorderSide(
            color: const Color(0x333B82F6), // blue-500/20
            width: 4,
          ),
        ),
        errorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(FlatRadius.md),
          borderSide: BorderSide(color: error, width: FlatBorder.focus),
        ),
        focusedErrorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(FlatRadius.md),
          borderSide: BorderSide(color: error, width: FlatBorder.focus),
        ),
        labelStyle: FlatTypography.bodySmall(brightness).copyWith(
          color: fg,
          fontWeight: FontWeight.w600,
        ),
        hintStyle: FlatTypography.bodySmall(brightness).copyWith(
          color: FlatColors.textTertiary(brightness),
        ),
      ),

      // ── Dividers — structural 2px ─────────────────────────────────
      dividerTheme: DividerThemeData(
        color: border,
        thickness: FlatBorder.structural,
        space: 0,
      ),

      // ── Sliders — flat ────────────────────────────────────────────
      sliderTheme: SliderThemeData(
        activeTrackColor: primary,
        inactiveTrackColor: muted,
        thumbColor: primary,
        overlayColor: primary.withValues(alpha: 0.12),
        trackHeight: 4,
        thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
        overlayShape: const RoundSliderOverlayShape(overlayRadius: 14),
      ),

      // ── Switch — macOS capsule style ───────────────────────────────
      switchTheme: SwitchThemeData(
        thumbColor: WidgetStateProperty.resolveWith((states) {
          // White thumb with shadow.
          return Colors.white;
        }),
        trackColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.selected)) {
            // macOS green when on.
            return brightness == Brightness.light
                ? const Color(0xFF34C759)
                : const Color(0xFF30D158);
          }
          // Gray when off.
          return brightness == Brightness.light
              ? const Color(0xFFE5E5EA)
              : const Color(0xFF636366);
        }),
        // Capsule shape.
        trackOutlineColor: WidgetStateProperty.all(Colors.transparent),
      ),

      // ── Tooltips — flat block ─────────────────────────────────────
      tooltipTheme: TooltipThemeData(
        decoration: BoxDecoration(
          color: fg,
          borderRadius: BorderRadius.circular(FlatRadius.md),
        ),
        textStyle: FlatTypography.caption(brightness).copyWith(color: bg),
        padding: const EdgeInsets.symmetric(
          horizontal: FlatSpacing.sm + 4,
          vertical: FlatSpacing.xs + 2,
        ),
      ),

      // ── List tiles ────────────────────────────────────────────────
      listTileTheme: ListTileThemeData(
        contentPadding: const EdgeInsets.symmetric(
          horizontal: FlatSpacing.md,
          vertical: FlatSpacing.xs,
        ),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(FlatRadius.md),
        ),
      ),

      // ── Bottom sheets — flat ──────────────────────────────────────
      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: surfaceContainerHigh,
        elevation: 0,
        shadowColor: Colors.transparent,
        surfaceTintColor: Colors.transparent,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(
            top: Radius.circular(FlatRadius.lg),
          ),
        ),
      ),

      // ── Dialog — flat ─────────────────────────────────────────────
      dialogTheme: DialogThemeData(
        backgroundColor: surfaceContainerHigh,
        elevation: 0,
        shadowColor: Colors.transparent,
        surfaceTintColor: Colors.transparent,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(FlatRadius.lg),
        ),
      ),

      // ── Tab bar — flat ────────────────────────────────────────────
      tabBarTheme: TabBarThemeData(
        indicator: UnderlineTabIndicator(
          borderSide: BorderSide(color: primary, width: FlatBorder.structural),
        ),
        labelColor: primary,
        unselectedLabelColor: FlatColors.textSecondary(brightness),
      ),
    );
  }
}
