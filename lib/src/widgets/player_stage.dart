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
    this.showSpectrum = true,
    this.circularCover = false,
    this.coverFraction = 0.84,
  });

  final Song song;
  final SpectrumMode mode;
  final RustFftSnapshot fft;
  final bool showSpectrum;
  final bool circularCover;
  final double coverFraction;

  @override
  Widget build(BuildContext context) {
    return AspectRatio(
      aspectRatio: 1,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final shortest = constraints.biggest.shortestSide;
          final coverSize = shortest * coverFraction.clamp(0.5, 0.98);
          return Stack(
            fit: StackFit.expand,
            children: [
              if (showSpectrum)
                Padding(
                  padding: const EdgeInsets.all(10),
                  child: CustomPaint(
                    painter: SpectrumPainter(
                      frequency: fft.frequency,
                      waveform: fft.waveform,
                      mode: mode,
                      color: Colors.white.withValues(alpha: 0.78),
                    ),
                  ),
                ),
              Center(
                child: Container(
                  width: coverSize,
                  height: coverSize,
                  decoration: BoxDecoration(
                    shape: circularCover ? BoxShape.circle : BoxShape.rectangle,
                    borderRadius:
                        circularCover ? null : BorderRadius.circular(26),
                    boxShadow: [
                      BoxShadow(
                        color: Colors.black.withValues(alpha: 0.45),
                        blurRadius: 28,
                        offset: const Offset(0, 16),
                      ),
                    ],
                    border: Border.all(
                      color: Colors.white.withValues(alpha: 0.14),
                    ),
                  ),
                  clipBehavior: Clip.antiAlias,
                  child: CoverArt(
                    width: coverSize,
                    height: coverSize,
                    coverHash: song.coverHash,
                    streamInfo: song.streamInfo,
                    size: CoverArtSize.mid,
                    shape: circularCover ? BoxShape.circle : BoxShape.rectangle,
                    borderRadius:
                        circularCover ? null : BorderRadius.circular(26),
                    placeholderIcon: PhosphorIcons.musicNote(),
                    placeholderIconSize: 34,
                  ).animate().fadeIn(duration: 280.ms),
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}
