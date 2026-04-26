import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../providers/providers.dart';
import '../rust/rust_api.dart';
import '../theme/bayin_tokens.dart';
import 'player_bar.dart';
import 'sidebar.dart';
import 'titlebar.dart';

/// Phase 2 shell. Composes:
///
/// ```
/// ┌───────── Titlebar (Win/Linux/macOS; no-op on mobile) ─────────┐
/// │                                                               │
/// │  ┌── Sidebar (docked) ──┐  ┌── body (child) ──────────────┐   │
/// │  │                      │  │                              │   │
/// │  │                      │  │                              │   │
/// │  └──────────────────────┘  └──────────────────────────────┘   │
/// ├─────────── PlayerBar ─────────────────────────────────────────┤
/// └───────────────────────────────────────────────────────────────┘
/// ```
///
/// On narrow layouts (medium / compact on mobile, or compact on desktop) the
/// sidebar collapses to an overlay drawer controlled by a local flag.
///
/// On `/player` the sidebar and player bar are hidden — that's the
/// full-screen player stage (Phase 5 polishes it).
class RootScaffold extends ConsumerStatefulWidget {
  const RootScaffold({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<RootScaffold> createState() => _RootScaffoldState();
}

class _RootScaffoldState extends ConsumerState<RootScaffold>
    with SingleTickerProviderStateMixin {
  bool _overlayOpen = false;
  ProviderSubscription<AsyncValue<List<RustFileWatchEvent>>>?
      _watchEventsSubscription;

  bool get _isDesktop =>
      !kIsWeb && (Platform.isWindows || Platform.isLinux || Platform.isMacOS);

  @override
  void initState() {
    super.initState();
    _watchEventsSubscription = ref.listenManual(
      fileWatcherEventsProvider,
      (previous, next) {
        next.whenData((events) {
          if (events.isEmpty) {
            return;
          }
          ref.invalidate(librarySongsProvider);
          ref.invalidate(libraryAlbumsProvider);
          ref.invalidate(libraryArtistsProvider);
        });
      },
    );
  }

  @override
  void dispose() {
    _watchEventsSubscription?.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final layout = ref.watch(responsiveLayoutProvider);
    final tokens = Theme.of(context).extension<BayinTokens>()!;
    final location = GoRouterState.of(context).uri.path;

    final isPlayer = location == '/player' || location.startsWith('/player/');
    final docked = !isPlayer && layout.sidebarMode == SidebarMode.docked;
    final showOverlaySidebar = !isPlayer && !docked && _overlayOpen;

    return Scaffold(
      backgroundColor: tokens.windowBg,
      body: Column(
        children: [
          if (_isDesktop) const AppTitlebar(),
          Expanded(
            child: Stack(
              children: [
                Row(
                  children: [
                    if (docked)
                      SizedBox(
                        width: kSidebarDockedWidth,
                        child: Sidebar(isOverlay: false),
                      ),
                    Expanded(
                      child: _BodySurface(
                        isPlayer: isPlayer,
                        onRequestOpenSidebar: docked || isPlayer
                            ? null
                            : () => setState(() => _overlayOpen = true),
                        child: widget.child,
                      ),
                    ),
                  ],
                ),
                if (!isPlayer && !docked) _OverlaySidebarScrim(
                      visible: showOverlaySidebar,
                      onTap: () => setState(() => _overlayOpen = false),
                    ),
                if (!isPlayer && !docked) _OverlaySidebar(
                      visible: showOverlaySidebar,
                      onClose: () => setState(() => _overlayOpen = false),
                    ),
              ],
            ),
          ),
          if (!isPlayer) const PlayerBar(),
        ],
      ),
    );
  }
}

class _BodySurface extends StatelessWidget {
  const _BodySurface({
    required this.isPlayer,
    required this.onRequestOpenSidebar,
    required this.child,
  });

  final bool isPlayer;
  final VoidCallback? onRequestOpenSidebar;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final tokens = Theme.of(context).extension<BayinTokens>()!;
    return Material(
      color: isPlayer ? tokens.playerBg : tokens.windowBg,
      child: Stack(
        children: [
          Positioned.fill(child: child),
          if (onRequestOpenSidebar != null)
            Positioned(
              top: 8,
              left: 8,
              child: SafeArea(
                child: IconButton.filledTonal(
                  tooltip: 'Open sidebar',
                  icon: const Icon(Icons.menu),
                  onPressed: onRequestOpenSidebar,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _OverlaySidebarScrim extends StatelessWidget {
  const _OverlaySidebarScrim({required this.visible, required this.onTap});

  final bool visible;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      ignoring: !visible,
      child: AnimatedOpacity(
        duration: const Duration(milliseconds: 200),
        opacity: visible ? 0.4 : 0,
        child: GestureDetector(
          onTap: onTap,
          child: Container(color: Colors.black),
        ),
      ),
    );
  }
}

class _OverlaySidebar extends StatelessWidget {
  const _OverlaySidebar({required this.visible, required this.onClose});

  final bool visible;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width * kSidebarOverlayWidthFraction;
    return AnimatedPositioned(
      duration: const Duration(milliseconds: 250),
      curve: Curves.easeOutCubic,
      top: 0,
      bottom: 0,
      left: visible ? 0 : -width,
      width: width,
      child: Material(
        elevation: 12,
        child: Sidebar(isOverlay: true, onNavigate: onClose),
      ),
    );
  }
}
