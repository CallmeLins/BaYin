import 'package:flutter/material.dart';

import '../models/models.dart';
import '../rust/rust_api.dart';
import 'spectrum/spectrum_painter.dart';

class PlayerStage extends StatelessWidget {
  const PlayerStage({
    super.key,
    required this.song,
    required this.mode,
    required this.fft,
  });

  final Song song;
  final SpectrumMode mode;
  final RustFftSnapshot fft;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return AspectRatio(
      aspectRatio: 1,
      child: DecoratedBox(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(24),
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [
              scheme.primary.withValues(alpha: 0.2),
              scheme.secondary.withValues(alpha: 0.16),
              scheme.surfaceContainerHighest.withValues(alpha: 0.6),
            ],
          ),
        ),
        child: Stack(
          fit: StackFit.expand,
          children: [
            Padding(
              padding: const EdgeInsets.all(12),
              child: CustomPaint(
                painter: SpectrumPainter(
                  frequency: fft.frequency,
                  waveform: fft.waveform,
                  mode: mode,
                  color: scheme.primary,
                ),
              ),
            ),
            Center(
              child: Container(
                width: 148,
                height: 148,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: scheme.surface.withValues(alpha: 0.86),
                  border: Border.all(
                    color: scheme.outlineVariant.withValues(alpha: 0.5),
                  ),
                ),
                alignment: Alignment.center,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 18),
                  child: Text(
                    song.title,
                    textAlign: TextAlign.center,
                    maxLines: 4,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      height: 1.2,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
