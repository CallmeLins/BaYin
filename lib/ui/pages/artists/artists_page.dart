import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — artists list.
class ArtistsPage extends ConsumerWidget {
  const ArtistsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final asyncArtists = ref.watch(libraryArtistsProvider);
    return asyncArtists.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load artists\n$error'),
      ),
      data: (artists) {
        if (artists.isEmpty) {
          return Center(
            child: Padding(
              padding: const EdgeInsets.all(32),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    PhosphorIcons.microphone(),
                    size: 48,
                    color: tokens.textSecondary,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'No artists yet',
                    style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Scan music to populate the library.',
                    style: TextStyle(
                      color: tokens.textSecondary,
                    ),
                  ),
                ],
              ),
            ),
          );
        }
        return Column(
          children: [
            BayinPageHeader(
              title: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('Artists'),
                  const SizedBox(width: 8),
                  Text(
                    '${artists.length}',
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
                child: GridView.builder(
                  gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                    maxCrossAxisExtent: 180,
                    mainAxisSpacing: 16,
                    crossAxisSpacing: 16,
                    childAspectRatio: 0.9,
                  ),
                  itemCount: artists.length,
                  itemBuilder: (context, index) {
                    return _ArtistCard(artist: artists[index]);
                  },
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}


class _ArtistCard extends ConsumerStatefulWidget {
  const _ArtistCard({required this.artist});

  final Artist artist;

  @override
  ConsumerState<_ArtistCard> createState() => _ArtistCardState();
}

class _ArtistCardState extends ConsumerState<_ArtistCard> {
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
        onTap: () => context.go('/artists/${Uri.encodeComponent(widget.artist.id)}'),
        onTapDown: (_) => setState(() => _pressing = true),
        onTapUp: (_) => setState(() => _pressing = false),
        onTapCancel: () => setState(() => _pressing = false),
        child: AnimatedScale(
          duration: const Duration(milliseconds: 150),
          curve: Curves.easeOut,
          scale: _pressing ? 0.98 : 1,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 150),
            curve: Curves.easeOut,
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
              mainAxisAlignment: MainAxisAlignment.start,
              children: [
                CoverArt(
                  width: 96,
                  height: 96,
                  coverHash: widget.artist.coverHash,
                  streamCoverUrl: widget.artist.streamCoverUrl,
                  size: CoverArtSize.small,
                  shape: BoxShape.circle,
                  placeholderIcon: PhosphorIcons.microphone(),
                  placeholderIconSize: 24,
                ),
                const SizedBox(height: 10),
                Text(
                  widget.artist.name,
                  style: const TextStyle(
                    fontSize: 13.5,
                    fontWeight: FontWeight.w600,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 5),
                Text(
                  '${widget.artist.songCount} songs',
                  style: TextStyle(
                    fontSize: 11,
                    color: tokens.textSecondary,
                  ),
                  textAlign: TextAlign.center,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
