import 'package:fluent_ui/fluent_ui.dart';
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
    required this.isPlaying,
    this.showSpectrum = true,
    this.isColorful = false,
    this.circularCover = false,
    this.coverFraction = 0.84,
  });

  final Song song;
  final SpectrumMode mode;
  final RustFftSnapshot fft;
  final bool isPlaying;
  final bool showSpectrum;
  final bool isColorful;
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
              Center(
                child: SizedBox(
                  width: coverSize,
                  height: coverSize,
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
                    fit: BoxFit.cover,
                  ).animate().fadeIn(duration: 240.ms),
                ),
              ),
              if (showSpectrum)
                Positioned.fill(
                  child: RepaintBoundary(
                    child: CustomPaint(
                      painter: SpectrumPainter(
                        frequency: fft.frequency,
                        waveform: fft.waveform,
                        mode: mode,
                        color: Colors.white.withValues(alpha: 0.84),
                        kind: SpectrumPainterKind.ring,
                        isPlaying: isPlaying,
                        isColorful: isColorful,
                        stateKey: 'player-stage-${mode.name}',
                      ),
                    ),
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}
