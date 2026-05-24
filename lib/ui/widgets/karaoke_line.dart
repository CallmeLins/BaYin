import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../providers/providers.dart';

class KaraokeLine extends ConsumerWidget {
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
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final baseStyle = TextStyle(
      fontSize: active ? 22 : 16,
      fontWeight: active ? FontWeight.w700 : FontWeight.w500,
      color: active
          ? tokens.textPrimary
          : tokens.textSecondary.withValues(alpha: 0.70),
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
                    ? const Color(0xFF60A5FA)
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
