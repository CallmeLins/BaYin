import 'package:flutter/material.dart';

class BayinGhostIconButton extends StatefulWidget {
  const BayinGhostIconButton({
    super.key,
    required this.icon,
    required this.tooltip,
    this.onTap,
    this.size = 36,
    this.iconSize = 18,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  final double size;
  final double iconSize;

  @override
  State<BayinGhostIconButton> createState() => _BayinGhostIconButtonState();
}

class _BayinGhostIconButtonState extends State<BayinGhostIconButton> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final hoverColor = brightness == Brightness.dark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.05);

    return Tooltip(
      message: widget.tooltip,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        cursor: widget.onTap == null
            ? SystemMouseCursors.basic
            : SystemMouseCursors.click,
        child: GestureDetector(
          onTap: widget.onTap,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 150),
            curve: Curves.easeOut,
            width: widget.size,
            height: widget.size,
            decoration: BoxDecoration(
              color: _hovering && widget.onTap != null
                  ? hoverColor
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(999),
            ),
            alignment: Alignment.center,
            child: Icon(widget.icon, size: widget.iconSize),
          ),
        ),
      ),
    );
  }
}
