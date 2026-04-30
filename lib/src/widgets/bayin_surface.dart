import 'package:flutter/material.dart';

class BayinGlassCard extends StatelessWidget {
  const BayinGlassCard({
    super.key,
    required this.child,
    this.padding,
    this.margin,
    this.borderRadius = 12,
  });

  final Widget child;
  final EdgeInsetsGeometry? padding;
  final EdgeInsetsGeometry? margin;
  final double borderRadius;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Container(
      margin: margin,
      padding: padding,
      decoration: BoxDecoration(
        color: dark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(borderRadius),
        border: Border.all(
          color: dark
              ? Colors.white.withValues(alpha: 0.07)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.6,
        ),
      ),
      child: child,
    );
  }
}

