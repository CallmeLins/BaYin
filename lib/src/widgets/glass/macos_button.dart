import 'package:flutter/material.dart';

import '../../theme/macos_design_tokens.dart';
import '../../theme/design_tokens.dart';

/// macOS Primary Push Button.
class MacosPrimaryButton extends StatefulWidget {
  const MacosPrimaryButton({
    super.key,
    required this.onPressed,
    required this.child,
  });

  final VoidCallback? onPressed;
  final Widget child;

  @override
  State<MacosPrimaryButton> createState() => _MacosPrimaryButtonState();
}

class _MacosPrimaryButtonState extends State<MacosPrimaryButton>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _scaleAnimation;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: MacosDesignTokens.durationMicro,
      vsync: this,
    );
    _scaleAnimation = Tween<double>(
      begin: 1.0,
      end: MacosDesignTokens.buttonTapScale,
    ).animate(CurvedAnimation(parent: _controller, curve: Curves.easeOut));
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final isLight = brightness == Brightness.light;
    final isEnabled = widget.onPressed != null;

    final gradientTop = isLight ? const Color(0xFF3B82F6) : const Color(0xFF60A5FA);
    final gradientBottom = isLight ? const Color(0xFF2563EB) : const Color(0xFF3B82F6);

    return AnimatedBuilder(
      animation: _scaleAnimation,
      builder: (context, child) {
        return Transform.scale(scale: _scaleAnimation.value, child: child);
      },
      child: GestureDetector(
        onTapDown: isEnabled ? (_) => _controller.forward() : null,
        onTapUp: isEnabled
            ? (_) {
                _controller.reverse();
                widget.onPressed?.call();
              }
            : null,
        onTapCancel: isEnabled ? () => _controller.reverse() : null,
        child: Opacity(
          opacity: isEnabled ? 1.0 : 0.5,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [gradientTop, gradientBottom],
              ),
              borderRadius: BorderRadius.circular(6),
              border: Border.all(
                color: Colors.white.withValues(alpha: 0.2),
                width: 0.5,
              ),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.2),
                  blurRadius: 4,
                  offset: const Offset(0, 2),
                ),
              ],
            ),
            child: DefaultTextStyle(
              style: FlatTypography.button(brightness).copyWith(color: Colors.white),
              child: widget.child,
            ),
          ),
        ),
      ),
    );
  }
}

/// macOS Secondary Button.
class MacosSecondaryButton extends StatefulWidget {
  const MacosSecondaryButton({
    super.key,
    required this.onPressed,
    this.icon,
    required this.label,
  });

  final VoidCallback? onPressed;
  final IconData? icon;
  final String label;

  @override
  State<MacosSecondaryButton> createState() => _MacosSecondaryButtonState();
}

class _MacosSecondaryButtonState extends State<MacosSecondaryButton>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _scaleAnimation;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: MacosDesignTokens.durationMicro,
      vsync: this,
    );
    _scaleAnimation = Tween<double>(
      begin: 1.0,
      end: MacosDesignTokens.buttonTapScale,
    ).animate(CurvedAnimation(parent: _controller, curve: Curves.easeOut));
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final isEnabled = widget.onPressed != null;

    return AnimatedBuilder(
      animation: _scaleAnimation,
      builder: (context, child) {
        return Transform.scale(scale: _scaleAnimation.value, child: child);
      },
      child: GestureDetector(
        onTapDown: isEnabled ? (_) => _controller.forward() : null,
        onTapUp: isEnabled
            ? (_) {
                _controller.reverse();
                widget.onPressed?.call();
              }
            : null,
        onTapCancel: isEnabled ? () => _controller.reverse() : null,
        child: Opacity(
          opacity: isEnabled ? 1.0 : 0.5,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            decoration: BoxDecoration(
              color: FlatColors.stateLayer(brightness, FlatStateIntensity.subtle),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(
                color: MacosBorder.color(brightness),
                width: MacosDesignTokens.hairlineWidth,
              ),
            ),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                if (widget.icon != null) ...[
                  Icon(widget.icon, size: 16, color: FlatColors.foreground(brightness)),
                  const SizedBox(width: 8),
                ],
                Text(widget.label, style: FlatTypography.button(brightness)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// macOS Icon Button.
class MacosIconButton extends StatefulWidget {
  const MacosIconButton({
    super.key,
    required this.icon,
    this.onPressed,
    this.size = 36,
  });

  final Widget icon;
  final VoidCallback? onPressed;
  final double size;

  @override
  State<MacosIconButton> createState() => _MacosIconButtonState();
}

class _MacosIconButtonState extends State<MacosIconButton>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _scaleAnimation;
  bool _isHovering = false;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: MacosDesignTokens.durationMicro,
      vsync: this,
    );
    _scaleAnimation = Tween<double>(begin: 1.0, end: 0.92).animate(
      CurvedAnimation(parent: _controller, curve: Curves.easeOut),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return AnimatedBuilder(
      animation: _scaleAnimation,
      builder: (context, child) {
        return Transform.scale(scale: _scaleAnimation.value, child: child);
      },
      child: MouseRegion(
        onEnter: (_) => setState(() => _isHovering = true),
        onExit: (_) => setState(() => _isHovering = false),
        child: GestureDetector(
          onTapDown: widget.onPressed != null ? (_) => _controller.forward() : null,
          onTapUp: widget.onPressed != null
              ? (_) {
                  _controller.reverse();
                  widget.onPressed?.call();
                }
              : null,
          onTapCancel: widget.onPressed != null ? () => _controller.reverse() : null,
          child: AnimatedContainer(
            duration: MacosDesignTokens.durationMicro,
            width: widget.size,
            height: widget.size,
            decoration: BoxDecoration(
              color: _isHovering
                  ? FlatColors.stateLayer(brightness, FlatStateIntensity.standard)
                  : Colors.transparent,
              shape: BoxShape.circle,
            ),
            child: Center(child: widget.icon),
          ),
        ),
      ),
    );
  }
}
