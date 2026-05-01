import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
import '../../utils/utils.dart';
import '../../widgets/widgets.dart';

/// Songs list page — the main library view.
class SongsPage extends ConsumerStatefulWidget {
  const SongsPage({super.key});

  @override
  ConsumerState<SongsPage> createState() => _SongsPageState();
}

class _SongsPageState extends ConsumerState<SongsPage> {
  final ScrollController _scrollController = ScrollController();
  String? _activeLetter;

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final tokens = ref.watch(bayinTokensProvider);
    final asyncSongs = ref.watch(librarySongsProvider);
    final brightness = Theme.of(context).brightness;

    return asyncSongs.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, stack) => _ErrorState(error: error),
      data: (songs) {
        if (songs.isEmpty) {
          return const _EmptyLibraryState();
        }
        // Compute bucket index for each song; the list is already sorted by
        // title in Rust (`ORDER BY title COLLATE NOCASE`).
        final buckets = <String, int>{};
        for (var i = 0; i < songs.length; i++) {
          final letter = firstPinyinLetter(songs[i].title);
          buckets.putIfAbsent(letter, () => i);
        }
        final available = buckets.keys.toSet();
        return Column(
          children: [
            BayinPageHeader(
              title: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('Songs'),
                  const SizedBox(width: FlatSpacing.sm),
                  Text(
                    '${songs.length}',
                    style: FlatTypography.caption(brightness),
                  ),
                ],
              ),
              right: Row(
                children: [
                  IconButton(
                    onPressed: () => context.go('/search'),
                    icon: Icon(PhosphorIcons.magnifyingGlass()),
                    tooltip: 'Search',
                  ),
                  IconButton(
                    onPressed: () => context.go('/scan'),
                    icon: Icon(PhosphorIcons.folderSimple()),
                    tooltip: 'Scan',
                  ),
                ],
              ),
            ),
            Expanded(
              child: Container(
                margin: EdgeInsets.zero,
                decoration: BoxDecoration(
                  color: tokens.isDark
                      ? FlatColors.surfaceAltDark
                      : FlatColors.mutedLight,
                  borderRadius: BorderRadius.circular(FlatRadius.md),
                  border: Border.all(
                    color: tokens.isDark
                        ? FlatColors.borderDark
                        : FlatColors.borderLight,
                    width: FlatBorder.structural,
                  ),
                ),
                child: Row(
                  children: [
                    Expanded(
                      child: SongList(
                        scrollController: _scrollController,
                        songs: songs,
                        showIndex: true,
                        onTap: (song) => ref
                            .read(playerControllerProvider.notifier)
                            .playQueue(songs, startIndex: songs.indexOf(song)),
                        onLongPress: (song) =>
                            SongMenu.show(context, song: song),
                      ),
                    ),
                    AlphabetScroller(
                      availableLetters: available,
                      currentLetter: _activeLetter,
                      onLetterTap: (letter) => _jumpToBucket(letter, buckets),
                    ),
                    const SizedBox(width: FlatSpacing.xs),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }

  void _jumpToBucket(String letter, Map<String, int> buckets) {
    final index = buckets[letter];
    if (index == null || !_scrollController.hasClients) return;
    setState(() => _activeLetter = letter);
    const itemExtent = 52.0;
    _scrollController.animateTo(
      index * itemExtent,
      duration: FlatDurations.standard,
      curve: FlatDurations.curve,
    );
  }
}

class _EmptyLibraryState extends ConsumerWidget {
  const _EmptyLibraryState();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(FlatSpacing.xl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.musicNotes(),
              size: 48,
              color: tokens.textSecondary,
            ),
            const SizedBox(height: FlatSpacing.sm + 4),
            Text(
              'Your library is empty',
              style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: FlatSpacing.xs + 2),
            Text(
              'Scan a folder to get started.',
              style: TextStyle(color: tokens.textSecondary),
            ),
            const SizedBox(height: FlatSpacing.md),
            FilledButton.icon(
              onPressed: () => context.go('/scan'),
              icon: Icon(PhosphorIcons.folderSimple(), size: 18),
              label: const Text('Scan music'),
            ),
            const SizedBox(height: FlatSpacing.sm),
            TextButton.icon(
              onPressed: () => context.go('/debug'),
              icon: Icon(PhosphorIcons.bug(), size: 18),
              label: const Text('Open FFI debug'),
            ),
          ],
        ),
      ),
    );
  }
}

class _ErrorState extends ConsumerWidget {
  const _ErrorState({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(FlatSpacing.xl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.warning(),
              size: 40,
              color: Colors.red,
            ),
            const SizedBox(height: FlatSpacing.sm + 4),
            Text(
              'Failed to load library',
              style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: FlatSpacing.xs + 2),
            SelectableText(
              '$error',
              style: TextStyle(color: tokens.textSecondary),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}
