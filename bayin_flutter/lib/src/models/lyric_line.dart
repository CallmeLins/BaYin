class KaraokeToken {
  const KaraokeToken({
    required this.text,
    required this.startMs,
    required this.durationMs,
  });

  final String text;
  final int startMs;
  final int durationMs;
}

class LyricLine {
  const LyricLine({
    required this.startMs,
    required this.text,
    this.tokens = const <KaraokeToken>[],
  });

  final int startMs;
  final String text;
  final List<KaraokeToken> tokens;
}
