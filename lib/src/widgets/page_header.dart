import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';

class BayinPageHeader extends ConsumerWidget {
  const BayinPageHeader({
    super.key,
    required this.title,
    this.left,
    this.right,
    this.layout = BayinPageHeaderLayout.centered,
    this.margin = const EdgeInsets.only(bottom: 8),
  });

  final Widget title;
  final Widget? left;
  final Widget? right;
  final BayinPageHeaderLayout layout;
  final EdgeInsetsGeometry margin;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final responsive = ref.watch(responsiveLayoutProvider);
    final tokens = Theme.of(context).extension<BayinTokens>()!;
    final isCompact = responsive.breakpoint == Breakpoint.compact;

    return Container(
      margin: margin,
      padding: EdgeInsets.fromLTRB(
        isCompact ? 12 : 12,
        isCompact ? 14 : 8,
        isCompact ? 12 : 12,
        isCompact ? 8 : 8,
      ),
      decoration: BoxDecoration(
        color: isCompact
            ? tokens.windowBg
            : tokens.titlebarBg.withValues(alpha: 0.82),
        border: Border(
          bottom: BorderSide(
            color: tokens.separatorColor,
            width: 0.5,
          ),
        ),
      ),
      child: SizedBox(
        height: 40,
        child: Row(
          children: [
            if (left != null) left!,
            if (left != null) const SizedBox(width: 8),
            Expanded(
              child: Align(
                alignment: Alignment.centerLeft,
                child: DefaultTextStyle(
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w600,
                    color: Theme.of(context).colorScheme.onSurface,
                    letterSpacing: -0.1,
                  ),
                  overflow: TextOverflow.ellipsis,
                  maxLines: 1,
                  child: title,
                ),
              ),
            ),
            if (right != null) const SizedBox(width: 8),
            if (right != null) right!,
          ],
        ),
      ),
    );
  }
}

enum BayinPageHeaderLayout {
  centered,
  fluid,
}
