import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../theme/bayin_tokens.dart';
import 'settings_provider.dart';

/// Provides flat [BayinTokens] based on the current theme mode.
///
/// These tokens determine the background colors for different surface regions
/// (window, sidebar, titlebar, player bar, popovers).
final bayinTokensProvider = Provider<BayinTokens>((ref) {
  final mode = ref.watch(themeModeProvider);
  return switch (mode) {
    ThemeMode.dark => darkTokens,
    _ => lightTokens,
  };
});
