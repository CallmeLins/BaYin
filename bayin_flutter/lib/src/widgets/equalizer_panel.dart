import 'package:flutter/material.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

class EqualizerPanel extends StatelessWidget {
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
    '31Hz',
    '62Hz',
    '125Hz',
    '250Hz',
    '500Hz',
    '1kHz',
    '2kHz',
    '4kHz',
    '8kHz',
    '16kHz',
  ];

  final bool enabled;
  final String selectedPreset;
  final List<double> gains;
  final ValueChanged<bool> onEnabledChanged;
  final ValueChanged<String> onPresetChanged;
  final Future<void> Function(int index, double value) onBandGainChanged;
  final VoidCallback onReset;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Container(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHigh,
            borderRadius: BorderRadius.circular(12),
          ),
          child: SwitchListTile.adaptive(
            value: enabled,
            onChanged: onEnabledChanged,
            title: const Text('Enable equalizer'),
            subtitle: const Text('Apply EQ gains during playback'),
          ),
        ),
        const SizedBox(height: 10),
        _PresetRow(
          selected: selectedPreset,
          onChanged: onPresetChanged,
          onReset: onReset,
        ),
        const SizedBox(height: 10),
        Container(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHigh,
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
  });

  final String selected;
  final ValueChanged<String> onChanged;
  final VoidCallback onReset;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
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
            icon: Icon(PhosphorIcons.arrowCounterClockwise()),
            tooltip: 'Reset',
            onPressed: onReset,
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
    return ChoiceChip(
      label: Text(label),
      selected: selected == id,
      onSelected: (_) => onChanged(id),
    );
  }
}

class _BandRow extends StatelessWidget {
  const _BandRow({
    required this.label,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final double value;
  final Future<void> Function(double) onChanged;

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
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      ],
    );
  }
}
