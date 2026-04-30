import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../utils/utils.dart';
import '../../widgets/widgets.dart';

class SearchPage extends ConsumerStatefulWidget {
  const SearchPage({super.key});

  @override
  ConsumerState<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends ConsumerState<SearchPage> {
  final TextEditingController _controller = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  final ScrollController _scrollController = ScrollController();
  String _activeQuery = '';
  String? _activeLetter;
  Timer? _debounce;

  List<Song>? _cachedSongs;
  final Map<String, String> _haystacks = <String, String>{};

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onQueryChanged);
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _controller.removeListener(_onQueryChanged);
    _controller.dispose();
    _focusNode.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _onQueryChanged() {
    _debounce?.cancel();
    final raw = _controller.text.trim();
    if (raw.isEmpty) {
      setState(() {
        _activeQuery = '';
        _activeLetter = null;
      });
      return;
    }
    _debounce = Timer(const Duration(milliseconds: 180), () {
      if (!mounted) return;
      setState(() {
        _activeQuery = raw.toLowerCase();
        _activeLetter = null;
      });
    });
  }

  void _clear() {
    _controller.clear();
    _focusNode.requestFocus();
  }

  void _ensureHaystacks(List<Song> songs) {
    if (identical(songs, _cachedSongs)) return;
    _cachedSongs = songs;
    _haystacks.clear();
    for (final s in songs) {
      _haystacks[s.id] = [
        searchHaystack(s.title),
        searchHaystack(s.artist),
        searchHaystack(s.album),
      ].join('|');
    }
  }

  List<Song> _filter(List<Song> songs) {
    if (_activeQuery.isEmpty) return const <Song>[];
    _ensureHaystacks(songs);
    return songs
        .where((s) => (_haystacks[s.id] ?? '').contains(_activeQuery))
        .toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final asyncSongs = ref.watch(librarySongsProvider);
    return Column(
      children: [
        _SearchBar(
          controller: _controller,
          focusNode: _focusNode,
          onClear: _clear,
        ),
        Expanded(
          child: asyncSongs.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (error, _) => Center(
              child: SelectableText('Failed to load library\n$error'),
            ),
            data: (songs) {
              if (_activeQuery.isEmpty) {
                return const _PromptState();
              }
              final hits = _filter(songs);
              if (hits.isEmpty) {
                return _EmptyResults(query: _controller.text);
              }
              return _ResultsView(
                songs: hits,
                scrollController: _scrollController,
                activeLetter: _activeLetter,
                onLetterChange: (letter) =>
                    setState(() => _activeLetter = letter),
              );
            },
          ),
        ),
      ],
    );
  }
}

class _SearchBar extends StatelessWidget {
  const _SearchBar({
    required this.controller,
    required this.focusNode,
    required this.onClear,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return BayinPageHeader(
      layout: BayinPageHeaderLayout.fluid,
      margin: const EdgeInsets.only(bottom: 6),
      title: Container(
        height: 38,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: scheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            Icon(
              PhosphorIcons.magnifyingGlass(),
              size: 18,
              color: scheme.onSurfaceVariant,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: TextField(
                controller: controller,
                focusNode: focusNode,
                autofocus: true,
                textInputAction: TextInputAction.search,
                decoration: const InputDecoration(
                  border: InputBorder.none,
                  isDense: true,
                  hintText: 'Search songs, artists, albums...',
                ),
                style: const TextStyle(fontSize: 14),
              ),
            ),
            ValueListenableBuilder<TextEditingValue>(
              valueListenable: controller,
              builder: (context, value, _) {
                if (value.text.isEmpty) return const SizedBox.shrink();
                return IconButton(
                  visualDensity: VisualDensity.compact,
                  onPressed: onClear,
                  icon: Icon(PhosphorIcons.x(), size: 16),
                  tooltip: 'Clear',
                );
              },
            ),
          ],
        ),
      ),
      right: TextButton(
        onPressed: () {
          if (context.canPop()) {
            context.pop();
          } else {
            context.go('/');
          }
        },
        child: const Text('Cancel'),
      ),
    );
  }
}

class _ResultsView extends ConsumerWidget {
  const _ResultsView({
    required this.songs,
    required this.scrollController,
    required this.activeLetter,
    required this.onLetterChange,
  });

  final List<Song> songs;
  final ScrollController scrollController;
  final String? activeLetter;
  final ValueChanged<String> onLetterChange;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    final sorted = [...songs]
      ..sort((a, b) {
        final la = firstPinyinLetter(a.title);
        final lb = firstPinyinLetter(b.title);
        if (la == lb) {
          return a.title.toLowerCase().compareTo(b.title.toLowerCase());
        }
        if (la == '#') return 1;
        if (lb == '#') return -1;
        return la.compareTo(lb);
      });

    final buckets = <String, int>{};
    for (var i = 0; i < sorted.length; i++) {
      final letter = firstPinyinLetter(sorted[i].title);
      buckets.putIfAbsent(letter, () => i);
    }
    final showScroller = sorted.length > 10;

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 6),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(
              '${sorted.length} result${sorted.length == 1 ? '' : 's'}',
              style: TextStyle(
                fontSize: 12,
                color: scheme.onSurfaceVariant,
              ),
            ),
          ),
        ),
        Expanded(
          child: Container(
            margin: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            decoration: BoxDecoration(
              color: Theme.of(context).brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.04)
                  : Colors.black.withValues(alpha: 0.02),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                color: Theme.of(context).brightness == Brightness.dark
                    ? Colors.white.withValues(alpha: 0.07)
                    : Colors.black.withValues(alpha: 0.06),
                width: 0.6,
              ),
            ),
            child: Row(
              children: [
                Expanded(
                  child: SongList(
                    scrollController: scrollController,
                    songs: sorted,
                    showIndex: false,
                    onTap: (song) => ref
                        .read(playerControllerProvider.notifier)
                        .playQueue(sorted, startIndex: sorted.indexOf(song)),
                    onLongPress: (song) => SongMenu.show(context, song: song),
                  ),
                ),
                if (showScroller) ...[
                  AlphabetScroller(
                    availableLetters: buckets.keys.toSet(),
                    currentLetter: activeLetter,
                    onLetterTap: (letter) {
                      final index = buckets[letter];
                      if (index == null || !scrollController.hasClients) return;
                      onLetterChange(letter);
                      const itemExtent = 52.0;
                      scrollController.animateTo(
                        index * itemExtent,
                        duration: const Duration(milliseconds: 180),
                        curve: Curves.easeOut,
                      );
                    },
                  ),
                  const SizedBox(width: 4),
                ],
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _PromptState extends StatelessWidget {
  const _PromptState();

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.magnifyingGlass(),
              size: 48,
              color: scheme.onSurfaceVariant.withValues(alpha: 0.6),
            ),
            const SizedBox(height: 12),
            Text(
              'Start typing to search your library',
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyResults extends StatelessWidget {
  const _EmptyResults({required this.query});

  final String query;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              PhosphorIcons.smileyMeh(),
              size: 40,
              color: scheme.onSurfaceVariant,
            ),
            const SizedBox(height: 12),
            Text(
              'No results for "$query"',
              style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: 4),
            Text(
              'Try a different keyword.',
              style: TextStyle(
                fontSize: 12,
                color: scheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

