import 'package:flutter/material.dart';

import '../theme/design_tokens.dart';

class BayinSectionHeader extends StatelessWidget {
  const BayinSectionHeader({
    super.key,
    required this.title,
    this.padding = const EdgeInsets.fromLTRB(14, 0, 14, 8),
  });

  final String title;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: padding,
      child: Row(
        children: [
          Text(title, style: FlatTypography.label(brightness)),
          const SizedBox(width: 8),
          Expanded(
            child: Divider(
              height: 1,
              thickness: 1,
              color: brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.10)
                  : Colors.black.withValues(alpha: 0.06),
            ),
          ),
        ],
      ),
    );
  }
}
