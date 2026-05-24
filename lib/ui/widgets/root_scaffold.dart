import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../providers/providers.dart';
import '../rust/rust_api.dart';
import '../theme/bayin_tokens.dart';
import 'player_bar.dart';
import 'sidebar.dart';
import 'titlebar.dart';

class RootScaffold extends ConsumerStatefulWidget {
  const RootScaffold({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<RootScaffold> createState() => _RootScaffoldState();
}

class _RootScaffoldState extends ConsumerState<RootScaffold> {
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
    final tokens = ref.watch(bayinTokensProvider);
    final location = GoRouterState.of(context).uri.path;

    final isPlayer = location == '/player' || location.startsWith('/player/');
    final sidebarDocked = !isPlayer && layout.sidebarMode == SidebarMode.docked;
    final sidebarOverlayCapable = !isPlayer && !sidebarDocked;
    final overlaySidebarShown = sidebarOverlayCapable && _overlayOpen;

    if (sidebarDocked && _overlayOpen) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        setState(() => _overlayOpen = false);
      });
    }

    return Container(
      color: tokens.windowBg,
      child: Column(
        children: [
          if (_isDesktop) const AppTitlebar(),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final overlayShift = constraints.maxWidth * 0.6;
                return Stack(
                  fit: StackFit.expand,
                  children: [
                    if (sidebarOverlayCapable)
                      _OverlaySidebar(
                        visible: overlaySidebarShown,
                        onClose: () => setState(() => _overlayOpen = false),
                      ),
                    AnimatedContainer(
                      duration: const Duration(milliseconds: 300),
                      curve: const Cubic(0.25, 1, 0.5, 1),
                      transform: Matrix4.translationValues(
                        overlaySidebarShown ? overlayShift : 0,
                        0,
                        0,
                      ),
                      child: Row(
                        children: [
                          if (sidebarDocked)
                            const SizedBox(
                              width: kSidebarDockedWidth,
                              child: Sidebar(isOverlay: false),
                            ),
                          Expanded(
                            child: GestureDetector(
                              behavior: HitTestBehavior.opaque,
                              onTap: overlaySidebarShown
                                  ? () => setState(() => _overlayOpen = false)
                                  : null,
                              child: _BodySurface(
                                isPlayer: isPlayer,
                                tokens: tokens,
                                onRequestOpenSidebar:
                                    sidebarOverlayCapable
                                        ? () => setState(() => _overlayOpen = true)
                                        : null,
                                child: widget.child,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                );
              },
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
    required this.tokens,
    required this.onRequestOpenSidebar,
    required this.child,
  });

  final bool isPlayer;
  final BayinTokens tokens;
  final VoidCallback? onRequestOpenSidebar;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      color: isPlayer ? tokens.playerBg : tokens.windowBg,
      child: Stack(
        children: [
          Positioned.fill(child: child),
          if (onRequestOpenSidebar != null)
            Positioned(
              top: 8,
              left: 8,
              child: SafeArea(
                child: IconButton(
                  onPressed: onRequestOpenSidebar,
                  icon: Icon(PhosphorIcons.list()),
                ),
              ),
            ),
        ],
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
      duration: const Duration(milliseconds: 300),
      curve: const Cubic(0.25, 1, 0.5, 1),
      top: 0,
      bottom: 0,
      left: visible ? 0 : -width,
      width: width,
      child: Sidebar(isOverlay: true, onNavigate: onClose),
    );
  }
}
