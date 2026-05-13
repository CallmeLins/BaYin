import 'package:flutter/material.dart';

import '../../theme/macos_design_tokens.dart';
import '../../theme/design_tokens.dart';

/// macOS Text Field.
class MacosTextField extends StatefulWidget {
  const MacosTextField({
    super.key,
    this.controller,
    this.placeholder,
    this.onChanged,
    this.onSubmitted,
    this.prefix,
    this.suffix,
    this.obscureText = false,
    this.enabled = true,
    this.maxLines = 1,
    this.autofocus = false,
  });

  final TextEditingController? controller;
  final String? placeholder;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;
  final Widget? prefix;
  final Widget? suffix;
  final bool obscureText;
  final bool enabled;
  final int maxLines;
  final bool autofocus;

  @override
  State<MacosTextField> createState() => _MacosTextFieldState();
}

class _MacosTextFieldState extends State<MacosTextField> {
  late FocusNode _focusNode;
  bool _isFocused = false;

  @override
  void initState() {
    super.initState();
    _focusNode = FocusNode();
    _focusNode.addListener(() {
      setState(() => _isFocused = _focusNode.hasFocus);
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final isLight = brightness == Brightness.light;

    final bgColor = isLight
        ? const Color(0x0A000000)
        : const Color(0x0DFFFFFF);

    final innerShadow = isLight
        ? const Color(0x0F000000)
        : const Color(0x1A000000);

    return Container(
      decoration: BoxDecoration(
        color: bgColor,
        borderRadius: BorderRadius.circular(5),
        boxShadow: [
          BoxShadow(
            color: innerShadow,
            blurRadius: 2,
            offset: const Offset(0, 1),
          ),
        ],
        border: Border.all(
          color: _isFocused
              ? const Color(0x333B82F6)
              : MacosBorder.color(brightness),
          width: _isFocused ? 4 : 0.5,
        ),
      ),
      child: Row(
        children: [
          if (widget.prefix != null)
            Padding(
              padding: const EdgeInsets.only(left: 12),
              child: widget.prefix!,
            ),
          Expanded(
            child: TextField(
              controller: widget.controller,
              focusNode: _focusNode,
              onChanged: widget.onChanged,
              onSubmitted: widget.onSubmitted,
              obscureText: widget.obscureText,
              enabled: widget.enabled,
              maxLines: widget.maxLines,
              autofocus: widget.autofocus,
              style: FlatTypography.body(brightness),
              decoration: InputDecoration(
                hintText: widget.placeholder,
                hintStyle: FlatTypography.body(brightness).copyWith(
                  color: FlatColors.textTertiary(brightness),
                ),
                border: InputBorder.none,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 10,
                ),
              ),
            ),
          ),
          if (widget.suffix != null)
            Padding(
              padding: const EdgeInsets.only(right: 12),
              child: widget.suffix!,
            ),
        ],
      ),
    );
  }
}
