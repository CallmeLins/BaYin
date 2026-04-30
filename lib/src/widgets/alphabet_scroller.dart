import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';
import '../utils/pinyin.dart';

class AlphabetScroller extends ConsumerWidget {
  const AlphabetScroller({
    super.key,
    required this.availableLetters,
    required this.onLetterTap,
    this.currentLetter,
  });

  final Set<String> availableLetters;
  final ValueChanged<String> onLetterTap;
  final String? currentLetter;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
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
              tokens: tokens,
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
    required this.tokens,
    required this.onTap,
  });

  final String letter;
  final bool isAvailable;
  final bool isActive;
  final BayinTokens tokens;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final Color color;
    if (isActive) {
      color = const Color(0xFF3B82F6);
    } else if (isAvailable) {
      color = tokens.textPrimary;
    } else {
      color = tokens.textPrimary.withValues(alpha: 0.3);
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
