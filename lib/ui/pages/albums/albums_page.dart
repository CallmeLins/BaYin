import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — local album grid.
class AlbumsPage extends ConsumerWidget {
  const AlbumsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final asyncAlbums = ref.watch(libraryAlbumsProvider);
    return asyncAlbums.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load albums\n$error'),
      ),
      data: (albums) {
        if (albums.isEmpty) {
          return _EmptyState(
            icon: PhosphorIcons.vinylRecord(),
            title: 'No albums yet',
            subtitle: 'Scan music to populate the library.',
          );
        }
        return Column(
          children: [
            BayinPageHeader(
              title: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('Albums'),
                  const SizedBox(width: 8),
                  Text(
                    '${albums.length}',
                    style: TextStyle(
                      fontSize: 13,
                      color: tokens.textSecondary,
                    ),
                  ),
                ],
              ),
              right: BayinGhostIconButton(
                icon: PhosphorIcons.magnifyingGlass(),
                tooltip: 'Search',
                onTap: () => context.go('/search'),
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 0, 8, 0),
                child: Container(
                  padding: const EdgeInsets.fromLTRB(10, 12, 10, 8),
                  decoration: BoxDecoration(
                    color: tokens.isDark
                        ? Colors.white.withValues(alpha: 0.04)
                        : Colors.black.withValues(alpha: 0.02),
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(
                      color: tokens.isDark
                          ? Colors.white.withValues(alpha: 0.07)
                          : Colors.black.withValues(alpha: 0.06),
                      width: 0.6,
                    ),
                  ),
                  child: GridView.builder(
                    gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                      maxCrossAxisExtent: 180,
                      mainAxisSpacing: 16,
                      crossAxisSpacing: 16,
                      childAspectRatio: 0.78,
                    ),
                    itemCount: albums.length,
                    itemBuilder: (context, index) {
                      return _AlbumCard(album: albums[index]);
                    },
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}


class _AlbumCard extends ConsumerStatefulWidget {
  const _AlbumCard({required this.album});

  final Album album;

  @override
  ConsumerState<_AlbumCard> createState() => _AlbumCardState();
}

class _AlbumCardState extends ConsumerState<_AlbumCard> {
  bool _hovering = false;
  bool _pressing = false;

  @override
  Widget build(BuildContext context) {
    final tokens = ref.watch(bayinTokensProvider);
    final brightness = Theme.of(context).brightness;

    return MouseRegion(
      onEnter: (_) => setState(() => _hovering = true),
      onExit: (_) => setState(() {
        _hovering = false;
        _pressing = false;
      }),
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: () => context.go('/albums/${Uri.encodeComponent(widget.album.id)}'),
        onTapDown: (_) => setState(() => _pressing = true),
        onTapUp: (_) => setState(() => _pressing = false),
        onTapCancel: () => setState(() => _pressing = false),
        child: AnimatedScale(
          duration: const Duration(milliseconds: 150),
          curve: Curves.easeOut,
          scale: _pressing ? 0.98 : 1,
          child: Container(
            padding: const EdgeInsets.all(4),
            decoration: BoxDecoration(
              color: _hovering
                  ? (brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.04)
                        : Colors.black.withValues(alpha: 0.025))
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(16),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                AspectRatio(
                  aspectRatio: 1,
                  child: CoverArt(
                    width: double.infinity,
                    height: double.infinity,
                    coverHash: widget.album.coverHash,
                    streamCoverUrl: widget.album.streamCoverUrl,
                    size: CoverArtSize.mid,
                    borderRadius: BorderRadius.circular(12),
                    placeholderIcon: PhosphorIcons.vinylRecord(),
                    placeholderIconSize: 40,
                  ),
                ),
                const SizedBox(height: 10),
                Text(
                  widget.album.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
                ),
                const SizedBox(height: 3),
                Text(
                  widget.album.artist,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 11,
                    color: tokens.textSecondary,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _EmptyState extends ConsumerWidget {
  const _EmptyState({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              icon,
              size: 48,
              color: tokens.textSecondary,
            ),
            const SizedBox(height: 12),
            Text(title, style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500)),
            const SizedBox(height: 6),
            Text(
              subtitle,
              style: TextStyle(
                color: tokens.textSecondary,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
