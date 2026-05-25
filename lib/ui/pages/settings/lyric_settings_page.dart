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
              const SizedBox(height: 16),

              // ── All Settings in One Card ─────────────────────────────
              BayinGlassGroup(
                child: Column(
                  children: [


                    // ── Font Size ────────────────────────────────────
                    _SliderRow(
                      title: 'Font Size',
                      value: app.lyricsFontSize,
                      min: 12,
                      max: 32,
                      onChanged: controller.setLyricsFontSize,
                    ),
                    _InsetDivider(),

                    // ── Translation Font Size ────────────────────────
                    _SliderRow(
                      title: 'Translation Font Size',
                      value: app.lyricsTranslationFontSize,
                      min: 10,
                      max: 28,
                      onChanged: controller.setLyricsTranslationFontSize,
                    ),
                    _InsetDivider(),

                    // ── Lyrics Offset ────────────────────────────────
                    _OffsetRow(
                      title: 'Lyrics Offset',
                      subtitle: 'Adjust lyrics timing to sync with audio.',
                      valueMs: app.lyricsOffsetMs,
                      onChanged: controller.setLyricsOffsetMs,
                    ),
                    _InsetDivider(),

                    // ── Lyric Position ───────────────────────────────
                    Padding(
                      padding: const EdgeInsets.all(14),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text(
                            'Lyric Position',
                            style: TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                          const SizedBox(height: 10),
                          _SegmentedSelector<String>(
                            options: const [
                              _SegmentOption(value: 'left', label: 'Left'),
                              _SegmentOption(value: 'center', label: 'Center'),
                              _SegmentOption(value: 'right', label: 'Right'),
                            ],
                            selected: app.lyricsPosition,
                            onChanged: controller.setLyricsPosition,
                          ),
                        ],
                      ),
                    ),
                    _InsetDivider(),

                    // ── Word-by-word Animation ───────────────────────
                    _SwitchRow(
                      title: 'Word-by-word animation',
                      subtitle: 'Animate karaoke tokens progressively.',
                      value: app.lyricsWordByWordAnimation,
                      onChanged: controller.setLyricsWordByWordAnimation,
                    ),
                    _InsetDivider(),

                    // ── Auto Blur ────────────────────────────────────
                    _SwitchRow(
                      title: 'Auto blur inactive lines',
                      subtitle: 'Reduce emphasis on non-active lyrics.',
                      value: app.lyricsAutoBlur,
                      onChanged: controller.setLyricsAutoBlur,
                    ),
                    _InsetDivider(),

                    // ── Selectable Lyrics ────────────────────────────
                    _SwitchRow(
                      title: 'Allow text selection',
                      subtitle: 'Allow selecting/copying lyric text.',
                      value: app.lyricsSelectable,
                      onChanged: controller.setLyricsSelectable,
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

// ── Segmented Selector ───────────────────────────────────────────────────

class _SegmentOption<T> {
  const _SegmentOption({required this.value, required this.label});

  final T value;
  final String label;
}

class _SegmentedSelector<T> extends StatelessWidget {
  const _SegmentedSelector({
    required this.options,
    required this.selected,
    required this.onChanged,
  });

  final List<_SegmentOption<T>> options;
  final T selected;
  final ValueChanged<T> onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.06)
            : Colors.black.withValues(alpha: 0.04),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        children: [
          for (var i = 0; i < options.length; i++) ...[
            if (i > 0) const SizedBox(width: 4),
            Expanded(
              child: GestureDetector(
                onTap: () => onChanged(options[i].value),
                child: Container(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  decoration: BoxDecoration(
                    color: selected == options[i].value
                        ? (brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.14)
                            : Colors.white)
                        : Colors.transparent,
                    borderRadius: BorderRadius.circular(8),
                    boxShadow: selected == options[i].value
                        ? [
                            BoxShadow(
                              color: Colors.black.withValues(alpha: 0.06),
                              blurRadius: 4,
                              offset: const Offset(0, 1),
                            ),
                          ]
                        : null,
                  ),
                  alignment: Alignment.center,
                  child: Text(
                    options[i].label,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                      color: selected == options[i].value
                          ? (brightness == Brightness.dark
                              ? Colors.white
                              : Colors.black)
                          : (brightness == Brightness.dark
                              ? Colors.white.withValues(alpha: 0.55)
                              : Colors.black.withValues(alpha: 0.50)),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

// ── Slider Row ───────────────────────────────────────────────────────────

class _SliderRow extends StatelessWidget {
  const _SliderRow({
    required this.title,
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
  });

  final String title;
  final double value;
  final double min;
  final double max;
  final Future<void> Function(double) onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                title,
                style: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
              Text(
                '${value.round()}px',
                style: TextStyle(
                  fontSize: 13,
                  color: brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.55)
                      : Colors.black.withValues(alpha: 0.50),
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          SliderTheme(
            data: SliderThemeData(
              trackHeight: 4,
              activeTrackColor: const Color(0xFF3B82F6),
              inactiveTrackColor: brightness == Brightness.dark
                  ? Colors.white.withValues(alpha: 0.12)
                  : Colors.black.withValues(alpha: 0.10),
              thumbColor: Colors.white,
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 7),
              overlayColor: const Color(0xFF3B82F6).withValues(alpha: 0.12),
            ),
            child: Slider(
              value: value.clamp(min, max),
              min: min,
              max: max,
              onChanged: (v) => onChanged(v),
            ),
          ),
        ],
      ),
    );
  }
}

// ── Offset Row ───────────────────────────────────────────────────────────

class _OffsetRow extends StatelessWidget {
  const _OffsetRow({
    required this.title,
    required this.subtitle,
    required this.valueMs,
    required this.onChanged,
  });

  final String title;
  final String subtitle;
  final int valueMs;
  final Future<void> Function(int) onChanged;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: TextStyle(
                      fontSize: 12,
                      color: brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.45)
                          : Colors.black.withValues(alpha: 0.40),
                    ),
                  ),
                ],
              ),
              Text(
                '${valueMs > 0 ? '+' : ''}$valueMs ms',
                style: TextStyle(
                  fontSize: 13,
                  color: brightness == Brightness.dark
                      ? Colors.white.withValues(alpha: 0.55)
                      : Colors.black.withValues(alpha: 0.50),
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              _OffsetButton(
                label: '-',
                onTap: () => onChanged(valueMs - 100),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Container(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.10)
                          : Colors.black.withValues(alpha: 0.08),
                      width: 0.8,
                    ),
                    color: brightness == Brightness.dark
                        ? Colors.white.withValues(alpha: 0.04)
                        : Colors.black.withValues(alpha: 0.03),
                  ),
                  alignment: Alignment.center,
                  child: Text(
                    '${valueMs > 0 ? '+' : ''}$valueMs ms',
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              _OffsetButton(
                label: '+',
                onTap: () => onChanged(valueMs + 100),
              ),
              const SizedBox(width: 8),
              _OffsetButton(
                label: 'Reset',
                onTap: () => onChanged(0),
                width: 56,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _OffsetButton extends StatefulWidget {
  const _OffsetButton({
    required this.label,
    required this.onTap,
    this.width = 36,
  });

  final String label;
  final VoidCallback onTap;
  final double width;

  @override
  State<_OffsetButton> createState() => _OffsetButtonState();
}

class _OffsetButtonState extends State<_OffsetButton> {
  bool _pressing = false;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return GestureDetector(
      onTap: widget.onTap,
      onTapDown: (_) => setState(() => _pressing = true),
      onTapUp: (_) => setState(() => _pressing = false),
      onTapCancel: () => setState(() => _pressing = false),
      child: AnimatedScale(
        duration: const Duration(milliseconds: 120),
        scale: _pressing ? 0.95 : 1.0,
        child: Container(
          width: widget.width,
          height: 36,
          decoration: BoxDecoration(
            color: brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.08)
                : Colors.black.withValues(alpha: 0.05),
            borderRadius: BorderRadius.circular(8),
          ),
          alignment: Alignment.center,
          child: Text(
            widget.label,
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
      ),
    );
  }
}

// ── Switch Row ───────────────────────────────────────────────────────────

class _SwitchRow extends StatelessWidget {
  const _SwitchRow({
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
    final brightness = Theme.of(context).brightness;
    return InkWell(
      onTap: () => onChanged(!value),
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: TextStyle(
                      fontSize: 12,
                      color: brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.45)
                          : Colors.black.withValues(alpha: 0.40),
                    ),
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

// ── Inset Divider ────────────────────────────────────────────────────────

class _InsetDivider extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Padding(
      padding: const EdgeInsets.only(left: 14),
      child: Divider(
        height: 1,
        thickness: 1,
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.06)
            : Colors.black.withValues(alpha: 0.06),
      ),
    );
  }
}
