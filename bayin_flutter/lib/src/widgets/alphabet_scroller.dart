import 'package:flutter/material.dart';

import '../utils/pinyin.dart';

/// Sticky A–Z + # rail. Taps emit the letter; the host list scrolls to the
/// first entry in that bucket.
class AlphabetScroller extends StatelessWidget {
  const AlphabetScroller({
    super.key,
    required this.availableLetters,
    required this.onLetterTap,
    this.currentLetter,
  });

  /// Which buckets actually exist in the current list (A, F, W, #, …). Letters
  /// outside this set render disabled.
  final Set<String> availableLetters;
  final ValueChanged<String> onLetterTap;
  final String? currentLetter;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: 20,
      padding: const EdgeInsets.symmetric(vertical: 8),
      alignment: Alignment.center,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final letter in alphabetBuckets)
            _LetterDot(
              letter: letter,
              isAvailable: availableLetters.contains(letter),
              isActive: currentLetter == letter,
              scheme: scheme,
              onTap: () => onLetterTap(letter),
            ),
        ],
      ),
    );
  }
}

class _LetterDot extends StatelessWidget {
  const _LetterDot({
    required this.letter,
    required this.isAvailable,
    required this.isActive,
    required this.scheme,
    required this.onTap,
  });

  final String letter;
  final bool isAvailable;
  final bool isActive;
  final ColorScheme scheme;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final Color color;
    if (isActive) {
      color = scheme.primary;
    } else if (isAvailable) {
      color = scheme.onSurface;
    } else {
      color = scheme.onSurface.withValues(alpha: 0.3);
    }
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: isAvailable ? onTap : null,
      child: Container(
        width: 18,
        height: 16,
        alignment: Alignment.center,
        child: Text(
          letter,
          style: TextStyle(
            fontSize: 10,
            fontWeight: isActive ? FontWeight.w700 : FontWeight.w500,
            color: color,
            fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
          ),
        ),
      ),
    );
  }
}
