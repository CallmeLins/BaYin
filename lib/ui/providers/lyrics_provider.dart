import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../rust/rust_api.dart';
import 'player_provider.dart';

final lyricsLinesProvider = FutureProvider<List<LyricLine>>((ref) async {
  final song = ref.watch(playerControllerProvider.select((state) => state.currentSong));
  if (song == null) {
    return const <LyricLine>[];
  }

  final rawLyrics = RustApi.instance.getLyrics(song.filePath);
  if (rawLyrics == null || rawLyrics.trim().isEmpty) {
    return const <LyricLine>[];
  }

  return parseLyricLines(rawLyrics);
});

final activeLyricIndexProvider = Provider<int?>((ref) {
  final lines = ref.watch(lyricsLinesProvider).valueOrNull ?? const <LyricLine>[];
  if (lines.isEmpty) {
    return null;
  }

  final positionSecs = ref.watch(
    playerControllerProvider.select((state) => state.positionSecs),
  );
  final positionMs = (positionSecs * 1000).round();

  for (var i = lines.length - 1; i >= 0; i--) {
    if (positionMs >= lines[i].startMs) {
      return i;
    }
  }
  return 0;
});

List<LyricLine> parseLyricLines(String rawLyrics) {
  final lines = <LyricLine>[];
  final timeTagRegex = RegExp(r'\[(\d{1,2}):(\d{2})(?:\.(\d{1,3}))?\]');
  final karaokeTokenRegex = RegExp(r'<(\d+),(\d+),\d+>([^<]+)');

  for (final rawLine in rawLyrics.split(RegExp(r'\r?\n'))) {
    final trimmed = rawLine.trim();
    if (trimmed.isEmpty) {
      continue;
    }

    final timeMatches = timeTagRegex.allMatches(trimmed).toList(growable: false);
    if (timeMatches.isEmpty) {
      continue;
    }

    final lineBody = trimmed.replaceAll(timeTagRegex, '').trim();
    final tokenMatches = karaokeTokenRegex.allMatches(lineBody).toList(growable: false);

    final tokens = <KaraokeToken>[
      for (final match in tokenMatches)
        KaraokeToken(
          text: match.group(3)?.trim() ?? '',
          startMs: int.tryParse(match.group(1) ?? '') ?? 0,
          durationMs: int.tryParse(match.group(2) ?? '') ?? 0,
        ),
    ].where((token) => token.text.isNotEmpty).toList(growable: false);

    final text = tokens.isNotEmpty
        ? tokens.map((token) => token.text).join()
        : lineBody;
    if (text.trim().isEmpty) {
      continue;
    }

    for (final match in timeMatches) {
      final minutes = int.tryParse(match.group(1) ?? '') ?? 0;
      final seconds = int.tryParse(match.group(2) ?? '') ?? 0;
      final fractionRaw = match.group(3) ?? '0';
      final millis = _fractionToMillis(fractionRaw);
      final startMs = minutes * 60000 + seconds * 1000 + millis;
      lines.add(LyricLine(startMs: startMs, text: text, tokens: tokens));
    }
  }

  lines.sort((a, b) => a.startMs.compareTo(b.startMs));
  return lines;
}

int _fractionToMillis(String fractionRaw) {
  if (fractionRaw.isEmpty) {
    return 0;
  }
  if (fractionRaw.length == 1) {
    return int.tryParse(fractionRaw) == null ? 0 : int.parse(fractionRaw) * 100;
  }
  if (fractionRaw.length == 2) {
    return int.tryParse(fractionRaw) == null ? 0 : int.parse(fractionRaw) * 10;
  }
  return int.tryParse(fractionRaw.substring(0, 3)) ?? 0;
}
