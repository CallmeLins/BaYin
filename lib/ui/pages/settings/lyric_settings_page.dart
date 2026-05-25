import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../theme/design_tokens.dart';
import '../../widgets/widgets.dart';

class LyricSettingsPage extends ConsumerWidget {
  const LyricSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final app = ref.watch(appSettingsProvider);
    final controller = ref.read(appSettingsProvider.notifier);
    final brightness = Theme.of(context).brightness;
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Lyric Settings'),
          left: BayinGhostIconButton(
            icon: PhosphorIcons.caretLeft(),
            tooltip: 'Back',
            onTap: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 12),
              _SectionGroup(
                title: 'TYPOGRAPHY & TIMING',
                child: Column(
                  children: [
                    _SliderTile(
                      title: 'Lyrics font size',
                      value: app.lyricsFontSize,
                      min: 12,
                      max: 36,
                      divisions: 24,
                      label: '${app.lyricsFontSize.toStringAsFixed(1)} px',
                      onChanged: controller.setLyricsFontSize,
                    ),
                    _FlatDivider(),
                    _SliderTile(
                      title: 'Translation font size',
                      value: app.lyricsTranslationFontSize,
                      min: 10,
                      max: 30,
                      divisions: 20,
                      label: '${app.lyricsTranslationFontSize.toStringAsFixed(1)} px',
                      onChanged: controller.setLyricsTranslationFontSize,
                    ),
                    _FlatDivider(),
                    _SliderTile(
                      title: 'Lyrics offset',
                      value: app.lyricsOffsetMs.toDouble(),
                      min: -3000,
                      max: 3000,
                      divisions: 120,
                      label: '${app.lyricsOffsetMs} ms',
                      onChanged: (value) => controller.setLyricsOffsetMs(value.round()),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 18),
              _SectionGroup(
                title: 'LAYOUT',
                child: Padding(
                  padding: const EdgeInsets.all(16),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        'Lyrics position',
                        style: FlatTypography.bodySmall(brightness).copyWith(
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                      SegmentedButton<String>(
                        segments: const [
                          ButtonSegment<String>(
                            value: 'left',
                            label: Text('Left'),
                          ),
                          ButtonSegment<String>(
                            value: 'center',
                            label: Text('Center'),
                          ),
                          ButtonSegment<String>(
                            value: 'right',
                            label: Text('Right'),
                          ),
                        ],
                        selected: {app.lyricsPosition},
                        onSelectionChanged: (v) {
                          controller.setLyricsPosition(v.first);
                        },
                      ),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 18),
              _SectionGroup(
                title: 'BEHAVIOR & ANIMATION',
                child: Column(
                  children: [
                    _SwitchTileCard(
                      title: 'Selectable lyrics',
                      subtitle: 'Allow selecting/copying lyric text.',
                      value: app.lyricsSelectable,
                      onChanged: controller.setLyricsSelectable,
                    ),
                    _FlatDivider(),
                    _SwitchTileCard(
                      title: 'Word-by-word animation',
                      subtitle: 'Animate karaoke tokens progressively.',
                      value: app.lyricsWordByWordAnimation,
                      onChanged: controller.setLyricsWordByWordAnimation,
                    ),
                    _FlatDivider(),
                    _SwitchTileCard(
                      title: 'Auto blur inactive lines',
                      subtitle: 'Reduce emphasis on non-active lyrics.',
                      value: app.lyricsAutoBlur,
                      onChanged: controller.setLyricsAutoBlur,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}


/// A structural 2px divider within a section group.
class _FlatDivider extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Divider(
      height: FlatBorder.structural,
      indent: FlatSpacing.md,
      color: FlatColors.border(brightness),
    );
  }
}

class _SectionGroup extends ConsumerWidget {
  const _SectionGroup({required this.title, required this.child});

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            FlatSpacing.md, 0, FlatSpacing.md, FlatSpacing.xs + 2,
          ),
          child: Text(
            title,
            style: FlatTypography.label(brightness),
          ),
        ),
        Container(
          decoration: BoxDecoration(
            color: FlatColors.background(brightness),
            borderRadius: BorderRadius.circular(FlatRadius.md),
            border: Border.all(
              color: FlatColors.border(brightness),
              width: FlatBorder.structural,
            ),
          ),
          child: child,
        ),
      ],
    );
  }
}

class _SliderTile extends ConsumerWidget {
  const _SliderTile({
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
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                title,
                style: FlatTypography.bodySmall(brightness).copyWith(
                  fontWeight: FontWeight.w500,
                ),
              ),
              Text(
                label,
                style: FlatTypography.caption(brightness),
              ),
            ],
          ),
          const SizedBox(height: 8),
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

class _SwitchTileCard extends ConsumerWidget {
  const _SwitchTileCard({
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
  Widget build(BuildContext context, WidgetRef ref) {
    final brightness = Theme.of(context).brightness;
    return InkWell(
      onTap: () => onChanged(!value),
      borderRadius: BorderRadius.circular(FlatRadius.md),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: FlatSpacing.md,
          vertical: FlatSpacing.sm + 4,
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: FlatTypography.bodySmall(brightness).copyWith(
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    subtitle,
                    style: FlatTypography.caption(brightness),
                  ),
                ],
              ),
            ),
            BayinToggleSwitch(
              value: value,
              onChanged: (v) => onChanged(v),
            ),
          ],
        ),
      ),
    );
  }
}
