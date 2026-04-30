import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../theme/bayin_tokens.dart';
import 'settings_provider.dart';

final bayinTokensProvider = Provider<BayinTokens>((ref) {
  final mode = ref.watch(themeModeProvider);
  return switch (mode) {
    ThemeMode.dark => _darkTokens,
    _ => _lightTokens,
  };
});

final _lightTokens = BayinTokens(
  windowBg: hslColor(0, 0, 100),
  titlebarBg: hslColor(240, 4.8, 96),
  sidebarBg: hslColor(240, 4.8, 96),
  barBg: hslColor(0, 0, 100),
  popoverBg: hslColor(0, 0, 100),
  playerBg: hslColor(240, 6, 7),
  separator: const Color(0xFFB0B0B0),
  separatorAlpha: 0.35,
  separatorSoftAlpha: 0.28,
  highlight: const Color(0xFFFFFFFF),
);

final _darkTokens = BayinTokens(
  windowBg: hslColor(240, 4, 2),
  titlebarBg: hslColor(240, 5, 4),
  sidebarBg: hslColor(240, 5, 5),
  barBg: hslColor(240, 5, 6),
  popoverBg: hslColor(240, 5, 8),
  playerBg: hslColor(240, 4, 2),
  separator: const Color(0xFFFFFFFF),
  separatorAlpha: 0.05,
  separatorSoftAlpha: 0.03,
  highlight: const Color(0xFFFFFFFF),
);
