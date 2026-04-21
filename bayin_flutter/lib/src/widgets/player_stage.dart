import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../models/models.dart';
import '../rust/rust_api.dart';
import 'cover_art.dart';
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
                  border: Border.all(
                    color: scheme.outlineVariant.withValues(alpha: 0.5),
                  ),
                ),
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    CoverArt(
                      width: 148,
                      height: 148,
                      coverHash: song.coverHash,
                      streamInfo: song.streamInfo,
                      size: CoverArtSize.mid,
                      shape: BoxShape.circle,
                      placeholderIcon: PhosphorIcons.musicNote(),
                      placeholderIconSize: 34,
                    ).animate().fadeIn(duration: 350.ms),
                    Align(
                      alignment: Alignment.bottomCenter,
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 10,
                          vertical: 8,
                        ),
                        decoration: BoxDecoration(
                          color: Colors.black.withValues(alpha: 0.35),
                          borderRadius: const BorderRadius.vertical(
                            bottom: Radius.circular(999),
                          ),
                        ),
                        child: Text(
                          song.title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                            fontSize: 12,
                            color: Colors.white,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
