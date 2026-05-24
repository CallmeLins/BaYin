import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'platform_provider.dart';

/// Mirrors `src-ui/src/constants/layout.ts` — shared viewport breakpoints.
class LayoutTokens {
  const LayoutTokens._();

  static const double tabletMinDim = 600;
  static const double bpMd = 768;
  static const double bpLg = 1024;
  static const double tabletPortraitDockedMinWidth = 840;
}

enum Breakpoint { compact, medium, wide }

enum Orientation { portrait, landscape }

enum SidebarMode { overlay, docked }

/// Mirrors `PlayerMode` in `src-ui/src/hooks/useResponsiveLayout.ts`.
enum PlayerMode { portraitSingle, phoneLandscapeControls, toggleSplit }

class ResponsiveLayout {
  const ResponsiveLayout({
    required this.size,
    required this.orientation,
    required this.breakpoint,
    required this.isTablet,
    required this.sidebarMode,
    required this.playerMode,
  });

  final Size size;
  final Orientation orientation;
  final Breakpoint breakpoint;
  final bool isTablet;
  final SidebarMode sidebarMode;
  final PlayerMode playerMode;

  double get minDim => size.shortestSide;
  double get width => size.width;
  double get height => size.height;
  bool get isCompact => breakpoint == Breakpoint.compact;
  bool get isMedium => breakpoint == Breakpoint.medium;
  bool get isWide => breakpoint == Breakpoint.wide;
}

Breakpoint _breakpointFor(double width) {
  if (width >= LayoutTokens.bpLg) return Breakpoint.wide;
  if (width >= LayoutTokens.bpMd) return Breakpoint.medium;
  return Breakpoint.compact;
}

/// Compute ResponsiveLayout for a given viewport + platform. Kept pure so it's
/// easy to unit test and so the provider can reuse the same rules the React
/// hook applies.
ResponsiveLayout computeResponsiveLayout({
  required Size size,
  required PlatformInfo platform,
}) {
  final orientation =
      size.width >= size.height ? Orientation.landscape : Orientation.portrait;
  final minDim = size.shortestSide;
  final isTablet = platform.isMobile && minDim >= LayoutTokens.tabletMinDim;
  final breakpoint = _breakpointFor(size.width);

  final SidebarMode sidebarMode;
  if (breakpoint == Breakpoint.wide) {
    sidebarMode = SidebarMode.docked;
  } else if (isTablet &&
      orientation == Orientation.portrait &&
      size.width >= LayoutTokens.tabletPortraitDockedMinWidth) {
    sidebarMode = SidebarMode.docked;
  } else if (breakpoint == Breakpoint.medium && platform.isDesktop) {
    sidebarMode = SidebarMode.docked;
  } else {
    sidebarMode = SidebarMode.overlay;
  }

  final PlayerMode playerMode;
  if (platform.isMobile && !isTablet && orientation == Orientation.landscape) {
    playerMode = PlayerMode.phoneLandscapeControls;
  } else if (breakpoint == Breakpoint.compact) {
    playerMode = PlayerMode.portraitSingle;
  } else if (isTablet && orientation == Orientation.portrait) {
    playerMode = PlayerMode.portraitSingle;
  } else {
    playerMode = PlayerMode.toggleSplit;
  }

  return ResponsiveLayout(
    size: size,
    orientation: orientation,
    breakpoint: breakpoint,
    isTablet: isTablet,
    sidebarMode: sidebarMode,
    playerMode: playerMode,
  );
}

/// State notifier that tracks the current window size. Drives Riverpod
/// consumers without forcing every widget to `MediaQuery.of(context)`.
class ResponsiveLayoutNotifier extends Notifier<ResponsiveLayout> {
  @override
  ResponsiveLayout build() {
    final platform = ref.watch(platformProvider);
    final view = WidgetsBinding.instance.platformDispatcher.views.first;
    final size = view.physicalSize / view.devicePixelRatio;
    return computeResponsiveLayout(size: size, platform: platform);
  }

  void update(Size size) {
    final platform = ref.read(platformProvider);
    state = computeResponsiveLayout(size: size, platform: platform);
  }
}

final responsiveLayoutProvider =
    NotifierProvider<ResponsiveLayoutNotifier, ResponsiveLayout>(
  ResponsiveLayoutNotifier.new,
);
