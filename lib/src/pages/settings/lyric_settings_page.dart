import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class LyricSettingsPage extends ConsumerWidget {
  const LyricSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final app = ref.watch(appSettingsProvider);
    final controller = ref.read(appSettingsProvider.notifier);
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Lyric Settings'),
          left: IconButton(
            tooltip: 'Back',
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
            icon: Icon(PhosphorIcons.caretLeft()),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 10),
        _SliderCard(
          title: 'Lyrics font size',
          value: app.lyricsFontSize,
          min: 12,
          max: 36,
          divisions: 24,
          label: '${app.lyricsFontSize.toStringAsFixed(1)} px',
          onChanged: controller.setLyricsFontSize,
        ),
        const SizedBox(height: 10),
        _SliderCard(
          title: 'Translation font size',
          value: app.lyricsTranslationFontSize,
          min: 10,
          max: 30,
          divisions: 20,
          label: '${app.lyricsTranslationFontSize.toStringAsFixed(1)} px',
          onChanged: controller.setLyricsTranslationFontSize,
        ),
        const SizedBox(height: 10),
        _SliderCard(
          title: 'Lyrics offset',
          value: app.lyricsOffsetMs.toDouble(),
          min: -3000,
          max: 3000,
          divisions: 120,
          label: '${app.lyricsOffsetMs} ms',
          onChanged: (value) => controller.setLyricsOffsetMs(value.round()),
        ),
        const SizedBox(height: 10),
        BayinGlassCard(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Lyrics position',
                style: TextStyle(fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 8),
              SegmentedButton<String>(
                segments: const [
                  ButtonSegment(value: 'left', label: Text('Left')),
                  ButtonSegment(value: 'center', label: Text('Center')),
                  ButtonSegment(value: 'right', label: Text('Right')),
                ],
                selected: <String>{app.lyricsPosition},
                onSelectionChanged: (value) {
                  if (value.isNotEmpty) {
                    controller.setLyricsPosition(value.first);
                  }
                },
              ),
            ],
          ),
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Selectable lyrics',
          subtitle: 'Allow selecting/copying lyric text.',
          value: app.lyricsSelectable,
          onChanged: controller.setLyricsSelectable,
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Word-by-word animation',
          subtitle: 'Animate karaoke tokens progressively.',
          value: app.lyricsWordByWordAnimation,
          onChanged: controller.setLyricsWordByWordAnimation,
        ),
        const SizedBox(height: 10),
        _SwitchCard(
          title: 'Auto blur inactive lines',
          subtitle: 'Reduce emphasis on non-active lyrics.',
          value: app.lyricsAutoBlur,
          onChanged: controller.setLyricsAutoBlur,
        ),
            ],
          ),
        ),
      ],
    );
  }
}

class _SliderCard extends StatelessWidget {
  const _SliderCard({
    required this.title,
    required this.value,
    required this.min,
    required this.max,
    required this.divisions,
    required this.label,
    required this.onChanged,
  });

  final String title;
  final double value;
  final double min;
  final double max;
  final int divisions;
  final String label;
  final Future<void> Function(double) onChanged;

  @override
  Widget build(BuildContext context) {
    return BayinGlassCard(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 4),
          Text(
            label,
            style: TextStyle(
              fontSize: 12,
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
          Slider(
            value: value.clamp(min, max),
            min: min,
            max: max,
            divisions: divisions,
            onChanged: (v) => onChanged(v),
          ),
        ],
      ),
    );
  }
}

class _SwitchCard extends StatelessWidget {
  const _SwitchCard({
    required this.title,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });

  final String title;
  final String subtitle;
  final bool value;
  final Future<void> Function(bool) onChanged;

  @override
  Widget build(BuildContext context) {
    return BayinGlassCard(
      child: SwitchListTile.adaptive(
        value: value,
        onChanged: (v) => onChanged(v),
        title: Text(title),
        subtitle: Text(subtitle),
      ),
    );
  }
}
