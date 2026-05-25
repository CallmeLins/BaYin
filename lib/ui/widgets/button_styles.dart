import 'package:flutter/material.dart';

class BayinButtonStyles {
  const BayinButtonStyles._();

  static ButtonStyle outlinedFlat(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return OutlinedButton.styleFrom(
      minimumSize: const Size(double.infinity, 40),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
      ),
    ).copyWith(
      overlayColor: WidgetStateProperty.resolveWith((states) {
        if (states.contains(WidgetState.pressed)) {
          return brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.12)
              : Colors.black.withValues(alpha: 0.10);
        }
        if (states.contains(WidgetState.hovered)) {
          return brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.10)
              : Colors.black.withValues(alpha: 0.06);
        }
        return null;
      }),
    );
  }

  static ButtonStyle filledFlat() {
    return FilledButton.styleFrom(
      minimumSize: const Size(double.infinity, 40),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
      ),
      elevation: 0,
    ).copyWith(
      overlayColor: WidgetStateProperty.resolveWith((states) {
        if (states.contains(WidgetState.pressed)) {
          return Colors.white.withValues(alpha: 0.12);
        }
        if (states.contains(WidgetState.hovered)) {
          return Colors.white.withValues(alpha: 0.08);
        }
        return null;
      }),
    );
  }
}
