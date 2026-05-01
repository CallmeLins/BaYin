import 'package:flutter/material.dart';

import '../theme/design_tokens.dart';

/// Shows a temporary notification using Material's SnackBar with flat design styling.
/// Replaces the previous fluent_ui displayInfoBar.
void showInfoMessage(
  BuildContext context,
  String message, {
  bool isError = false,
  Duration duration = const Duration(seconds: 3),
}) {
  final messenger = ScaffoldMessenger.maybeOf(context);
  if (messenger == null) return;

  final brightness = Theme.of(context).brightness;
  final bg = FlatColors.foreground(brightness);
  final fg = FlatColors.background(brightness);
  final accent = isError
      ? FlatColors.error(brightness)
      : FlatColors.secondary(brightness);

  messenger.showSnackBar(
    SnackBar(
      content: Row(
        children: [
          Icon(
            isError ? Icons.error_outline : Icons.check_circle_outline,
            size: 18,
            color: accent,
          ),
          const SizedBox(width: FlatSpacing.sm),
          Expanded(
            child: Text(
              message,
              style: TextStyle(color: fg, fontSize: 13),
            ),
          ),
        ],
      ),
      backgroundColor: bg,
      behavior: SnackBarBehavior.floating,
      margin: const EdgeInsets.all(FlatSpacing.md),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(FlatRadius.md),
      ),
      duration: duration,
      elevation: 0,
      padding: const EdgeInsets.symmetric(
        horizontal: FlatSpacing.md,
        vertical: FlatSpacing.sm + 4,
      ),
    ),
  );
}
