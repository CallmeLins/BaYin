import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../theme/design_tokens.dart';

/// Flat surface card — a solid color block with no shadows, no borders.
///
/// Replaces the previous BayinGlassCard. The flat design system uses
/// pure color blocks to define card boundaries rather than simulated depth.
class BayinSurface extends ConsumerWidget {
  const BayinSurface({
    super.key,
    required this.child,
    this.padding,
    this.margin,
    this.color,
    this.borderRadius,
  });

  final Widget child;
  final EdgeInsetsGeometry? padding;
  final EdgeInsetsGeometry? margin;
  final Color? color;
  final double? borderRadius;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    final bg = color ?? FlatColors.background(brightness);

    return Container(
      margin: margin,
      padding: padding ?? const EdgeInsets.all(FlatSpacing.md),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(borderRadius ?? FlatRadius.md),
      ),
      child: child,
    );
  }
}

/// A muted variant of [BayinSurface] — used for secondary content blocks.
class BayinMutedSurface extends ConsumerWidget {
  const BayinMutedSurface({
    super.key,
    required this.child,
    this.padding,
    this.margin,
    this.borderRadius,
  });

  final Widget child;
  final EdgeInsetsGeometry? padding;
  final EdgeInsetsGeometry? margin;
  final double? borderRadius;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return BayinSurface(
      padding: padding,
      margin: margin,
      borderRadius: borderRadius,
      color: FlatColors.muted(brightness),
      child: child,
    );
  }
}

/// A tinted surface using a soft color variant — for feature cards.
class BayinTintedSurface extends ConsumerWidget {
  const BayinTintedSurface({
    super.key,
    required this.child,
    required this.tint,
    this.padding,
    this.margin,
    this.borderRadius,
  });

  final Widget child;
  final Color tint;
  final EdgeInsetsGeometry? padding;
  final EdgeInsetsGeometry? margin;
  final double? borderRadius;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Blend the tint with the background at low opacity for a subtle effect.
    final brightness = Theme.of(context).brightness;
    final base = FlatColors.background(brightness);
    final blended = Color.alphaBlend(tint.withValues(alpha: 0.08), base);

    return BayinSurface(
      padding: padding,
      margin: margin,
      borderRadius: borderRadius,
      color: blended,
      child: child,
    );
  }
}

// ── Backward compatibility ──────────────────────────────────────────────

/// Deprecated — use [BayinSurface] or [BayinMutedSurface] instead.
///
/// Kept for backward compatibility with pages that haven't been
/// migrated to the flat design system yet.
@Deprecated('Use BayinSurface or BayinMutedSurface instead.')
class BayinGlassCard extends ConsumerWidget {
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
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Container(
      margin: margin,
      padding: padding,
      decoration: BoxDecoration(
        color: FlatColors.muted(brightness),
        borderRadius: BorderRadius.circular(borderRadius),
      ),
      child: child,
    );
  }
}
