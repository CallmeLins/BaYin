import 'package:flutter/material.dart';

class BayinGlassGroup extends StatelessWidget {
  const BayinGlassGroup({
    super.key,
    required this.child,
    this.radius = 16,
    this.borderWidth = 0.8,
  });

  final Widget child;
  final double radius;
  final double borderWidth;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(radius),
        border: Border.all(
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.08)
              : Colors.black.withValues(alpha: 0.06),
          width: borderWidth,
        ),
      ),
      child: child,
    );
  }
}
