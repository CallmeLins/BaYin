import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';
import 'toggle_switch.dart';

class EqualizerPanel extends ConsumerWidget {
  const EqualizerPanel({
    super.key,
    required this.enabled,
    required this.selectedPreset,
    required this.gains,
    required this.onEnabledChanged,
    required this.onPresetChanged,
    required this.onBandGainChanged,
    required this.onReset,
  });

  static const List<String> bands = [
    '31', '62', '125', '250', '500',
    '1k', '2k', '4k', '8k', '16k',
  ];

  final bool enabled;
  final String selectedPreset;
  final List<double> gains;
  final ValueChanged<bool> onEnabledChanged;
  final ValueChanged<String> onPresetChanged;
  final Future<void> Function(int index, double value) onBandGainChanged;
  final VoidCallback onReset;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final brightness = Theme.of(context).brightness;
    final surfaceColor = tokens.isDark
        ? Colors.white.withValues(alpha: 0.06)
        : Colors.black.withValues(alpha: 0.04);

    return Column(
      children: [
        // ── Enable Toggle ─────────────────────────────────────────────
        Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: surfaceColor,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('Enable equalizer', style: TextStyle(fontWeight: FontWeight.w600)),
                    const SizedBox(height: 4),
                    Text('Apply EQ gains during playback',
                        style: TextStyle(fontSize: 12, color: tokens.textSecondary)),
                  ],
                ),
              ),
              BayinToggleSwitch(
                value: enabled,
                onChanged: onEnabledChanged,
              ),
            ],
          ),
        ),
        const SizedBox(height: 10),

        // ── Preset Row ────────────────────────────────────────────────
        _PresetRow(
          selected: selectedPreset,
          onChanged: onPresetChanged,
          onReset: onReset,
          tokens: tokens,
        ),
        const SizedBox(height: 10),

        // ── EQ Grid ───────────────────────────────────────────────────
        Container(
          decoration: BoxDecoration(
            color: surfaceColor,
            borderRadius: BorderRadius.circular(12),
          ),
          padding: const EdgeInsets.fromLTRB(12, 16, 12, 12),
          child: Column(
            children: [
              // ── dB Labels ─────────────────────────────────────────
              Row(
                children: [
                  const SizedBox(width: 40),
                  Expanded(
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Text('+12', style: TextStyle(fontSize: 10, color: tokens.textSecondary)),
                        Text('0', style: TextStyle(fontSize: 10, color: tokens.textSecondary)),
                        Text('-12', style: TextStyle(fontSize: 10, color: tokens.textSecondary)),
                      ],
                    ),
                  ),
                  const SizedBox(width: 48),
                ],
              ),
              const SizedBox(height: 6),



              // ── Band Sliders ──────────────────────────────────────
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // ── Vertical dB Scale ───────────────────────────
                  SizedBox(
                    width: 40,
                    height: 160,
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Text('+12', style: TextStyle(fontSize: 9, color: tokens.textSecondary)),
                        Text('0', style: TextStyle(fontSize: 9, color: tokens.textSecondary)),
                        Text('-12', style: TextStyle(fontSize: 9, color: tokens.textSecondary)),
                      ],
                    ),
                  ),

                  // ── Sliders ─────────────────────────────────────
                  Expanded(
                    child: SizedBox(
                      height: 160,
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          for (var i = 0; i < gains.length && i < bands.length; i++) ...[
                            if (i > 0)
                              Container(
                                width: 1,
                                color: brightness == Brightness.dark
                                    ? Colors.white.withValues(alpha: 0.06)
                                    : Colors.black.withValues(alpha: 0.06),
                              ),
                            Expanded(
                              child: _BandSlider(
                                label: bands[i],
                                value: gains[i],
                                onChanged: (v) => onBandGainChanged(i, v),
                                enabled: enabled,
                              ),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),

                  // ── Value Display ───────────────────────────────
                  SizedBox(
                    width: 48,
                    height: 160,
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Text(
                          '${gains.isNotEmpty ? gains[0].toStringAsFixed(1) : '0.0'} dB',
                          style: TextStyle(
                            fontSize: 10,
                            color: tokens.textSecondary,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),

              const SizedBox(height: 8),

              // ── Frequency Labels ─────────────────────────────────
              Row(
                children: [
                  const SizedBox(width: 40),
                  Expanded(
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceAround,
                      children: [
                        for (final band in bands)
                          Text(
                            band,
                            style: TextStyle(
                              fontSize: 10,
                              color: tokens.textSecondary,
                            ),
                          ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 48),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

// ── Band Slider ──────────────────────────────────────────────────────────

class _BandSlider extends StatelessWidget {
  const _BandSlider({
    required this.label,
    required this.value,
    required this.onChanged,
    required this.enabled,
  });

  final String label;
  final double value;
  final Future<void> Function(double) onChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    final normalizedValue = ((value.clamp(-12, 12) + 12) / 24).clamp(0.0, 1.0);

    return GestureDetector(
      onVerticalDragUpdate: enabled
          ? (details) {
              final box = context.findRenderObject() as RenderBox?;
              if (box == null) return;
              final local = box.globalToLocal(details.globalPosition);
              final fraction = (1 - (local.dy / box.size.height)).clamp(0.0, 1.0);
              final newValue = (fraction * 24 - 12).clamp(-12.0, 12.0);
              onChanged(newValue);
            }
          : null,
      onTapDown: enabled
          ? (details) {
              final box = context.findRenderObject() as RenderBox?;
              if (box == null) return;
              final local = box.globalToLocal(details.globalPosition);
              final fraction = (1 - (local.dy / box.size.height)).clamp(0.0, 1.0);
              final newValue = (fraction * 24 - 12).clamp(-12.0, 12.0);
              onChanged(newValue);
            }
          : null,
      child: Container(
        color: Colors.transparent,
        child: CustomPaint(
          painter: _BandPainter(
            value: normalizedValue,
            brightness: brightness,
            enabled: enabled,
          ),
        ),
      ),
    );
  }
}

// ── Band Painter ─────────────────────────────────────────────────────────

class _BandPainter extends CustomPainter {
  _BandPainter({
    required this.value,
    required this.brightness,
    required this.enabled,
  });

  final double value;
  final Brightness brightness;
  final bool enabled;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height * (1 - value));
    final trackColor = brightness == Brightness.dark
        ? Colors.white.withValues(alpha: 0.10)
        : Colors.black.withValues(alpha: 0.08);
    final fillColor = enabled
        ? const Color(0xFF3B82F6)
        : (brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.25)
            : Colors.black.withValues(alpha: 0.20));

    // ── Track ───────────────────────────────────────────────────────
    final trackPaint = Paint()
      ..color = trackColor
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(
      Offset(size.width / 2, 0),
      Offset(size.width / 2, size.height),
      trackPaint,
    );

    // ── Filled portion ──────────────────────────────────────────────
    final fillPaint = Paint()
      ..color = fillColor
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(
      Offset(size.width / 2, size.height),
      center,
      fillPaint,
    );

    // ── Thumb ───────────────────────────────────────────────────────
    final thumbPaint = Paint()
      ..color = enabled ? Colors.white : (brightness == Brightness.dark
          ? Colors.white.withValues(alpha: 0.6)
          : Colors.black.withValues(alpha: 0.6))
      ..style = PaintingStyle.fill;
    canvas.drawCircle(center, 5, thumbPaint);

    // ── Thumb border ────────────────────────────────────────────────
    final borderPaint = Paint()
      ..color = fillColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    canvas.drawCircle(center, 5, borderPaint);
  }

  @override
  bool shouldRepaint(covariant _BandPainter oldDelegate) =>
      oldDelegate.value != value ||
      oldDelegate.brightness != brightness ||
      oldDelegate.enabled != enabled;
}

// ── Preset Row ───────────────────────────────────────────────────────────

class _PresetRow extends StatelessWidget {
  const _PresetRow({
    required this.selected,
    required this.onChanged,
    required this.onReset,
    required this.tokens,
  });

  final String selected;
  final ValueChanged<String> onChanged;
  final VoidCallback onReset;
  final BayinTokens tokens;

  @override
  Widget build(BuildContext context) {
    final surfaceColor = tokens.isDark
        ? Colors.white.withValues(alpha: 0.06)
        : Colors.black.withValues(alpha: 0.04);

    return Container(
      decoration: BoxDecoration(
        color: surfaceColor,
        borderRadius: BorderRadius.circular(12),
      ),
      padding: const EdgeInsets.all(10),
      child: Row(
        children: [
          const Text('Preset'),
          const SizedBox(width: 10),
          Expanded(
            child: Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                _chip('balance', 'Balance', selected, onChanged),
                _chip('bass', 'Bass', selected, onChanged),
                _chip('vocal', 'Vocal', selected, onChanged),
                _chip('bright', 'Bright', selected, onChanged),
              ],
            ),
          ),
          IconButton(
            onPressed: onReset,
            icon: Icon(PhosphorIcons.arrowCounterClockwise()),
          ),
        ],
      ),
    );
  }

  Widget _chip(
    String id,
    String label,
    String selected,
    ValueChanged<String> onChanged,
  ) {
    final active = selected == id;
    final isDark = tokens.isDark;
    return GestureDetector(
      onTap: () => onChanged(id),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(
          color: active
              ? (isDark ? Colors.white.withValues(alpha: 0.16) : Colors.black.withValues(alpha: 0.10))
              : (isDark ? Colors.white.withValues(alpha: 0.06) : Colors.black.withValues(alpha: 0.04)),
          borderRadius: BorderRadius.circular(999),
        ),
        child: Text(label, style: const TextStyle(fontSize: 13)),
      ),
    );
  }
}
