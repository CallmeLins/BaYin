import 'package:lpinyin/lpinyin.dart';

/// Pinyin helpers for CN/EN mixed library search + alphabet jump.
///
/// Naming convention: only covers `A`–`Z` and `#` (the bucket for anything
/// starting with a digit, symbol, or CJK char we can't romanize).
String firstPinyinLetter(String input) {
  if (input.isEmpty) return '#';
  final trimmed = input.trim();
  if (trimmed.isEmpty) return '#';

  final romanized = PinyinHelper.getFirstWordPinyin(trimmed);
  if (romanized.isEmpty) return '#';
  final first = romanized[0].toUpperCase();
  final code = first.codeUnitAt(0);
  if (code >= 0x41 && code <= 0x5A) return first;
  return '#';
}

/// Full pinyin form, useful for search matching.
String toPinyinString(String input, {String separator = ''}) {
  if (input.isEmpty) return '';
  return PinyinHelper.getPinyin(input, separator: separator).toLowerCase();
}

/// Lightweight haystack: concatenates the original text + its full-pinyin form
/// + first-letter-only form, all lowercased. Search can then do a simple
/// `contains`.
String searchHaystack(String input) {
  if (input.isEmpty) return '';
  final lower = input.toLowerCase();
  final full = toPinyinString(input);
  final shortcut =
      PinyinHelper.getShortPinyin(input).toLowerCase();
  return '$lower|$full|$shortcut';
}

/// Returns true when [query] matches [text] via plain substring, full pinyin,
/// or first-letter shortcut. Query is lowercased before matching.
bool matchPinyin(String text, String query) {
  if (query.isEmpty) return true;
  if (text.isEmpty) return false;
  return searchHaystack(text).contains(query.toLowerCase());
}

const List<String> alphabetBuckets = <String>[
  '#',
  'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
  'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
