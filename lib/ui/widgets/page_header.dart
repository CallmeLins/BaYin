import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/providers.dart';
import '../theme/design_tokens.dart' hide Breakpoint, computeBreakpoint, isWideBreakpoint;

/// Page header — aligned to Tauri web toolbar style.
class BayinPageHeader extends ConsumerWidget {
  const BayinPageHeader({
    super.key,
    required this.title,
    this.left,
    this.right,
    this.margin = const EdgeInsets.only(bottom: FlatSpacing.sm),
  });

  final Widget title;
  final Widget? left;
  final Widget? right;
  final EdgeInsetsGeometry margin;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final responsive = ref.watch(responsiveLayoutProvider);
    final isCompact = responsive.breakpoint == Breakpoint.compact;
    final brightness = Theme.of(context).brightness;

    final hPad = FlatSpacing.smPlus;
    final vPad = isCompact ? 10.0 : 8.0;

    return Container(
      margin: margin,
      padding: EdgeInsets.fromLTRB(hPad, vPad, hPad, vPad),
      decoration: BoxDecoration(
        color: FlatColors.surfaceContainer(brightness).withValues(
          alpha: isCompact ? 1.0 : 0.82,
        ),
        border: Border(
          bottom: BorderSide(
            color: FlatColors.border(brightness).withValues(alpha: 0.5),
            width: 1,
          ),
        ),
      ),
      child: SizedBox(
        height: 40,
        child: Row(
          children: [
            ?left,
            if (left != null) const SizedBox(width: FlatSpacing.sm),
            Expanded(
              child: Align(
                alignment: Alignment.centerLeft,
                child: DefaultTextStyle(
                  style: FlatTypography.headingCompact(brightness).copyWith(
                    fontSize: 16,
                    fontWeight: FontWeight.w600,
                  ),
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                  child: title,
                ),
              ),
            ),
            if (right != null) const SizedBox(width: FlatSpacing.sm),
            ?right,
          ],
        ),
      ),
    );
  }
}
