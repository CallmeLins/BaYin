import 'dart:ui';

import 'package:flutter/material.dart';

import '../../theme/macos_design_tokens.dart';

/// Glass Container — macOS-style frosted glass material.
class MacosGlassContainer extends StatelessWidget {
  const MacosGlassContainer({
    super.key,
    this.materialType = GlassMaterialType.thick,
    required this.child,
    this.borderRadius,
    this.showBezelHighlight = true,
    this.padding,
    this.margin,
  });

  final Widget child;
  final GlassMaterialType materialType;
  final BorderRadius? borderRadius;
  final bool showBezelHighlight;
  final EdgeInsetsGeometry? padding;
  final EdgeInsetsGeometry? margin;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final radius = borderRadius ?? BorderRadius.circular(12);

    final color = switch (materialType) {
      GlassMaterialType.thin => MacosGlass.thin(brightness),
      GlassMaterialType.thick => MacosGlass.thick(brightness),
      GlassMaterialType.ultra => MacosGlass.ultra(brightness),
    };

    final blurRadius = switch (materialType) {
      GlassMaterialType.thin => MacosGlass.thinBlur,
      GlassMaterialType.thick => MacosGlass.thickBlur,
      GlassMaterialType.ultra => MacosGlass.ultraBlur,
    };

    return Container(
      margin: margin,
      child: ClipRRect(
        borderRadius: radius,
        child: BackdropFilter(
          filter: ImageFilter.blur(
            sigmaX: blurRadius / 2,
            sigmaY: blurRadius / 2,
          ),
          child: Container(
            decoration: BoxDecoration(
              color: color,
              borderRadius: radius,
              border: Border.all(
                color: MacosBorder.color(brightness),
                width: MacosDesignTokens.hairlineWidth,
              ),
              boxShadow: MacosShadow.card(brightness),
            ),
            padding: padding,
            child: Stack(
              children: [
                if (showBezelHighlight)
                  Positioned(
                    top: 0,
                    left: 0,
                    right: 0,
                    height: 1,
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.vertical(
                          top: radius.topLeft,
                        ),
                        gradient: LinearGradient(
                          begin: Alignment.topCenter,
                          end: Alignment.bottomCenter,
                          colors: [
                            MacosBorder.bezelHighlight(brightness),
                            Colors.transparent,
                          ],
                        ),
                      ),
                    ),
                  ),
                child,
              ],
            ),
          ),
        ),
      ),
    );
  }
}
