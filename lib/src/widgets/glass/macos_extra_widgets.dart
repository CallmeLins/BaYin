import 'package:flutter/material.dart';

import '../../theme/macos_design_tokens.dart';
import '../../theme/design_tokens.dart';

/// macOS Toggle Switch.
class MacosSwitch extends StatefulWidget {
  const MacosSwitch({
    super.key,
    required this.value,
    required this.onChanged,
  });

  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  State<MacosSwitch> createState() => _MacosSwitchState();
}

class _MacosSwitchState extends State<MacosSwitch>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _thumbPosition;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: MacosDesignTokens.durationStandard,
      vsync: this,
    );
    _thumbPosition = Tween<double>(
      begin: widget.value ? 1.0 : 0.0,
      end: widget.value ? 1.0 : 0.0,
    ).animate(CurvedAnimation(parent: _controller, curve: Curves.easeOut));
  }

  @override
  void didUpdateWidget(MacosSwitch oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.value != oldWidget.value) {
      if (widget.value) {
        _controller.forward();
      } else {
        _controller.reverse();
      }
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final activeColor = brightness == Brightness.light
        ? const Color(0xFF34C759)
        : const Color(0xFF30D158);
    final inactiveColor = brightness == Brightness.light
        ? const Color(0xFFE5E5EA)
        : const Color(0xFF636366);

    return GestureDetector(
      onTap: widget.onChanged != null
          ? () => widget.onChanged!(!widget.value)
          : null,
      child: AnimatedBuilder(
        animation: _thumbPosition,
        builder: (context, child) {
          return Container(
            width: 51,
            height: 31,
            decoration: BoxDecoration(
              color: Color.lerp(inactiveColor, activeColor, _thumbPosition.value),
              borderRadius: BorderRadius.circular(15.5),
            ),
            child: Padding(
              padding: const EdgeInsets.all(2),
              child: Align(
                alignment: Alignment(_thumbPosition.value * 2 - 1, 0),
                child: Container(
                  width: 27,
                  height: 27,
                  decoration: BoxDecoration(
                    color: Colors.white,
                    shape: BoxShape.circle,
                    boxShadow: [
                      BoxShadow(
                        color: Colors.black.withValues(alpha: 0.15),
                        blurRadius: 4,
                        offset: const Offset(0, 2),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// macOS Slider.
class MacosSlider extends StatelessWidget {
  const MacosSlider({
    super.key,
    required this.value,
    required this.min,
    required this.max,
    this.divisions,
    required this.onChanged,
  });

  final double value;
  final double min;
  final double max;
  final int? divisions;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;

    return SliderTheme(
      data: SliderThemeData(
        activeTrackColor: FlatColors.primary(brightness),
        inactiveTrackColor: FlatColors.stateLayer(
          brightness,
          FlatStateIntensity.standard,
        ),
        thumbColor: FlatColors.primary(brightness),
        overlayColor: FlatColors.primary(brightness).withValues(alpha: 0.12),
        trackHeight: 4,
        thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 7),
        overlayShape: const RoundSliderOverlayShape(overlayRadius: 16),
      ),
      child: Slider(
        value: value,
        min: min,
        max: max,
        divisions: divisions,
        onChanged: onChanged,
      ),
    );
  }
}

/// macOS Badge.
class MacosBadge extends StatelessWidget {
  const MacosBadge({
    super.key,
    required this.label,
    required this.color,
  });

  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: color,
        ),
      ),
    );
  }
}
