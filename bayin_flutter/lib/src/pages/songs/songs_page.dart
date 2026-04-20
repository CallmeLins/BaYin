import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../utils/utils.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — real local songs list.
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
    final asyncSongs = ref.watch(librarySongsProvider);
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
            _Header(count: songs.length),
            Expanded(
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
                  const SizedBox(width: 4),
                ],
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
      duration: const Duration(milliseconds: 180),
      curve: Curves.easeOut,
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 8),
      child: Row(
        children: [
          Text(
            'Songs',
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const SizedBox(width: 8),
          Text(
            '$count',
            style: TextStyle(
              fontSize: 14,
              color: scheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

class _EmptyLibraryState extends ConsumerWidget {
  const _EmptyLibraryState();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.musicNotes(),
              size: 48,
              color: scheme.onSurfaceVariant,
            ),
            const SizedBox(height: 12),
            Text(
              'Your library is empty',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 6),
            Text(
              'Scan a folder to get started.',
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
            const SizedBox(height: 16),
            FilledButton.icon(
              icon: Icon(PhosphorIcons.folderSimple()),
              label: const Text('Scan music'),
              onPressed: () => context.go('/scan'),
            ),
            const SizedBox(height: 8),
            TextButton.icon(
              icon: Icon(PhosphorIcons.bug()),
              label: const Text('Open FFI debug'),
              onPressed: () => context.go('/debug'),
            ),
          ],
        ),
      ),
    );
  }
}

class _ErrorState extends StatelessWidget {
  const _ErrorState({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.warning(),
              size: 40,
              color: Theme.of(context).colorScheme.error,
            ),
            const SizedBox(height: 12),
            Text(
              'Failed to load library',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 6),
            SelectableText(
              '$error',
              style: TextStyle(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}
