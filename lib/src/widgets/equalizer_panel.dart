import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../providers/providers.dart';
import '../theme/bayin_tokens.dart';

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
    '31Hz', '62Hz', '125Hz', '250Hz', '500Hz',
    '1kHz', '2kHz', '4kHz', '8kHz', '16kHz',
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
    final surfaceColor = tokens.isDark
        ? Colors.white.withValues(alpha: 0.06)
        : Colors.black.withValues(alpha: 0.04);

    return Column(
      children: [
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
              ToggleSwitch(
                checked: enabled,
                onChanged: onEnabledChanged,
              ),
            ],
          ),
        ),
        const SizedBox(height: 10),
        _PresetRow(
          selected: selectedPreset,
          onChanged: onPresetChanged,
          onReset: onReset,
          tokens: tokens,
        ),
        const SizedBox(height: 10),
        Container(
          decoration: BoxDecoration(
            color: surfaceColor,
            borderRadius: BorderRadius.circular(12),
          ),
          padding: const EdgeInsets.fromLTRB(10, 10, 10, 4),
          child: Column(
            children: [
              for (var i = 0; i < gains.length && i < bands.length; i++)
                _BandRow(
                  label: bands[i],
                  value: gains[i],
                  onChanged: (value) => onBandGainChanged(i, value),
                  tokens: tokens,
                ),
            ],
          ),
        ),
      ],
    );
  }
}

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

class _BandRow extends StatelessWidget {
  const _BandRow({
    required this.label,
    required this.value,
    required this.onChanged,
    required this.tokens,
  });

  final String label;
  final double value;
  final Future<void> Function(double) onChanged;
  final BayinTokens tokens;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(
          width: 52,
          child: Text(label),
        ),
        Expanded(
          child: Slider(
            value: value.clamp(-12, 12),
            min: -12,
            max: 12,
            divisions: 48,
            label: '${value.toStringAsFixed(1)} dB',
            onChanged: (v) => onChanged(v),
          ),
        ),
        SizedBox(
          width: 48,
          child: Text(
            value.toStringAsFixed(1),
            textAlign: TextAlign.right,
            style: TextStyle(
              fontSize: 12,
              color: tokens.textSecondary,
            ),
          ),
        ),
      ],
    );
  }
}
