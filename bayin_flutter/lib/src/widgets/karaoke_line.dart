import 'package:flutter/material.dart';

import '../models/models.dart';

class KaraokeLine extends StatelessWidget {
  const KaraokeLine({
    super.key,
    required this.line,
    required this.positionMs,
    required this.active,
  });

  final LyricLine line;
  final int positionMs;
  final bool active;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final baseStyle = TextStyle(
      fontSize: active ? 20 : 16,
      fontWeight: active ? FontWeight.w700 : FontWeight.w500,
      color: active
          ? scheme.onSurface
          : scheme.onSurfaceVariant.withValues(alpha: 0.72),
      height: 1.35,
    );

    if (line.tokens.isEmpty) {
      return Text(
        line.text,
        textAlign: TextAlign.center,
        style: baseStyle,
      );
    }

    return RichText(
      textAlign: TextAlign.center,
      text: TextSpan(
        children: [
          for (final token in line.tokens)
            TextSpan(
              text: token.text,
              style: baseStyle.copyWith(
                color: _isTokenActive(token)
                    ? Theme.of(context).colorScheme.primary
                    : baseStyle.color,
              ),
            ),
        ],
      ),
    );
  }

  bool _isTokenActive(KaraokeToken token) {
    final local = positionMs - line.startMs;
    return local >= token.startMs && local <= token.startMs + token.durationMs;
  }
}
