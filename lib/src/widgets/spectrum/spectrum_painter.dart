import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:fluent_ui/fluent_ui.dart';

import '../../models/models.dart';

enum SpectrumPainterKind {
  ring,
  bottom,
}

class SpectrumPainter extends CustomPainter {
  SpectrumPainter({
    required this.frequency,
    required this.waveform,
    required this.mode,
    required this.color,
    this.kind = SpectrumPainterKind.ring,
    this.isPlaying = false,
    this.isColorful = false,
    this.stateKey = 'default',
  });

  static const int _fftSize = 128;
  static final Map<String, _SpectrumRuntimeState> _runtime = <String, _SpectrumRuntimeState>{};

  final List<int> frequency;
  final List<int> waveform;
  final SpectrumMode mode;
  final Color color;
  final SpectrumPainterKind kind;
  final bool isPlaying;
  final bool isColorful;
  final String stateKey;

  @override
  void paint(Canvas canvas, Size size) {
    if (size.width < 2 || size.height < 2) return;

    final state = _runtime.putIfAbsent(stateKey, _SpectrumRuntimeState.new);
    final nowMs = DateTime.now().microsecondsSinceEpoch / 1000.0;
    final dtSec = state.lastFrameMs <= 0
        ? (1 / 60.0)
        : _clamp((nowMs - state.lastFrameMs) / 1000.0, 0.001, 0.05);
    state.lastFrameMs = nowMs;
    final frames = dtSec * 60.0;

    _resampleToFixed(frequency, state.freqLinear, gamma: 1.0);
    _resampleToFixed(frequency, state.freq, gamma: 0.65);
    _resampleToFixed(waveform, state.wave, gamma: 1.0);

    if (isPlaying) {
      _smooth(state.smoothFreq, state.freq, 0.28);
      _smooth(state.smoothFreqLinear, state.freqLinear, 0.22);
      _smooth(state.smoothWave, state.wave, 0.20);
    } else {
      final decay = math.pow(0.92, frames).toDouble();
      for (var i = 0; i < _fftSize; i++) {
        state.smoothFreq[i] *= decay;
        state.smoothFreqLinear[i] *= decay;
        state.smoothWave[i] *= decay;
      }
    }
    state.rotation += (isPlaying ? 0.18 : 0.10) * dtSec;

    switch (kind) {
      case SpectrumPainterKind.bottom:
        _drawBottom(canvas, size, state, frames);
        return;
      case SpectrumPainterKind.ring:
        _drawRing(canvas, size, state, frames, nowMs);
        return;
    }
  }

  @override
  bool shouldRepaint(covariant SpectrumPainter oldDelegate) {
    return oldDelegate.mode != mode ||
        oldDelegate.kind != kind ||
        oldDelegate.isPlaying != isPlaying ||
        oldDelegate.isColorful != isColorful ||
        oldDelegate.stateKey != stateKey ||
        oldDelegate.frequency != frequency ||
        oldDelegate.waveform != waveform ||
        oldDelegate.color != color;
  }

  void _drawRing(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
    double nowMs,
  ) {
    switch (mode) {
      case SpectrumMode.godRing:
        _drawGodRing(canvas, size, state, frames, nowMs);
        return;
      case SpectrumMode.diffusionRing:
        _drawDiffusionRing(canvas, size, state, frames, nowMs);
        return;
      case SpectrumMode.attachmentRing:
        _drawAttachmentRing(canvas, size, state, frames, nowMs);
        return;
      case SpectrumMode.rotatingCover:
        _drawRotatingCover(canvas, size, state, frames);
        return;
      case SpectrumMode.trippyRipple:
        _drawTrippyRipple(canvas, size, state, frames, nowMs);
        return;
      case SpectrumMode.wave:
      case SpectrumMode.bessel:
      case SpectrumMode.columnar:
        _drawGodRing(canvas, size, state, frames, nowMs);
        return;
    }
  }

  void _drawGodRing(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
    double nowMs,
  ) {
    final w = size.width;
    final h = size.height;
    final center = Offset(w * 0.5, h * 0.5);
    final canvasR = math.min(w, h) * 0.5;
    final coverR = canvasR * 0.70;
    final maxR = math.max(2.0, canvasR - 2.0);
    final gap = math.max(14.0, coverR * 0.10);
    final bandStart = math.min(maxR - 2.0, coverR + gap);
    final band = math.max(0.0, maxR - bandStart);
    if (band < 6) return;

    final smoothed = state.smoothFreq;
    final hue = _themeHue(smoothed);

    final beat = _detectBeat(smoothed);
    if (isPlaying && beat && nowMs - state.lastBeatMs > 220) {
      state.lastBeatMs = nowMs;
      state.pulse = 1.0;
    } else {
      state.pulse = math.max(0.0, state.pulse - (frames / 60.0) * 2.8);
    }
    final pulse = state.pulse;

    final ringCount = band > math.min(w, h) * 0.22
        ? 3
        : (band > math.min(w, h) * 0.13 ? 2 : 1);

    final rings = ringCount == 3
        ? const <_GodRingCfg>[
            _GodRingCfg(t: 0.15, bars: 32, bandStart: 0.00, bandEnd: 0.28, speed: 0.55),
            _GodRingCfg(t: 0.52, bars: 64, bandStart: 0.15, bandEnd: 0.75, speed: -0.40),
            _GodRingCfg(t: 0.86, bars: 128, bandStart: 0.55, bandEnd: 1.00, speed: -0.33),
          ]
        : ringCount == 2
            ? const <_GodRingCfg>[
                _GodRingCfg(t: 0.25, bars: 48, bandStart: 0.05, bandEnd: 0.55, speed: 0.45),
                _GodRingCfg(t: 0.78, bars: 96, bandStart: 0.35, bandEnd: 1.00, speed: -0.33),
              ]
            : const <_GodRingCfg>[
                _GodRingCfg(t: 0.55, bars: 140, bandStart: 0.10, bandEnd: 1.00, speed: 0.35),
              ];

    canvas.save();
    canvas.translate(center.dx, center.dy);

    for (var ri = 0; ri < rings.length; ri++) {
      final ring = rings[ri];
      final baseR = bandStart + band * ring.t * (1 + (ri == rings.length - 1 ? 0.10 * pulse : 0));
      final thickness = _clamp((math.pi * 2 * baseR) / ring.bars * 0.55, 2.0, 6.0);
      final maxLen = math.min(maxR - baseR, math.max(math.min(w, h) * 0.08, band * 0.45));
      final minLen = math.max(6.0, maxLen * 0.10);
      final rot = state.rotation * (0.9 + ri * 0.15) + ring.speed * (isPlaying ? 1 : 0.5) * (nowMs * 0.001);

      for (var i = 0; i < ring.bars; i++) {
        final t = i / ring.bars;
        final raw = isPlaying ? _sampleLogBand(smoothed, t, ring.bandStart, ring.bandEnd) : 0.0;
        final v = _clamp01(raw + (isPlaying ? 0 : _idleNoise(nowMs, (i + ri * 97).toDouble())));
        final len = minLen + v * v * maxLen;
        final alpha = _clamp01(0.30 + v * 0.70 + (ri == rings.length - 1 ? 0.12 * pulse : 0.0));

        final paint = Paint()
          ..style = PaintingStyle.stroke
          ..strokeCap = StrokeCap.round
          ..strokeWidth = thickness
          ..color = _hsla(hue, 0.90, 0.65, alpha);

        final a = t * math.pi * 2 + rot;
        final p1 = Offset(math.cos(a) * baseR, math.sin(a) * baseR);
        final p2 = Offset(math.cos(a) * (baseR + len), math.sin(a) * (baseR + len));
        canvas.drawLine(p1, p2, paint);
      }
    }
    canvas.restore();
  }

  void _drawDiffusionRing(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
    double nowMs,
  ) {
    final center = Offset(size.width * 0.5, size.height * 0.5);
    final canvasR = math.min(size.width, size.height) * 0.5;
    final startR = canvasR * 0.22;
    final maxR = math.max(startR + 2.0, canvasR - 2.0);

    if (!isPlaying) {
      if (state.ripples.isEmpty) {
        state.ripples.add(_Ripple(
          r: startR,
          a: 0.22,
          phase: math.Random().nextDouble() * math.pi * 2,
          wobble: 2.0,
        ));
      }
      state.ripples.length = 1;
      final idle = state.ripples.first;
      idle.r += (1.4 + 0.5 * _idleNoise(nowMs, 17.0)) * frames;
      idle.a = _lerp(idle.a, 0.22, 0.08 * frames);
      if (idle.r > maxR) {
        idle.r = startR;
        idle.a = 0;
      }
    } else {
      if (nowMs - state.lastDiffusionSpawnMs > 1200) {
        state.lastDiffusionSpawnMs = nowMs;
        final bass = _clamp01(state.smoothFreq[2] * 1.1);
        state.ripples.add(_Ripple(
          r: startR,
          a: 0.35 + bass * 0.15,
          phase: math.Random().nextDouble() * math.pi * 2,
          wobble: _clamp(math.min(size.width, size.height) * 0.004 * (0.5 + bass), 0.8, 3.2),
        ));
        while (state.ripples.length > 3) {
          state.ripples.removeAt(0);
        }
      }
    }

    final hue = _themeHue(state.smoothFreq);
    final next = <_Ripple>[];
    for (final ripple in state.ripples) {
      if (isPlaying) {
        final energy = _energy(state.smoothFreq);
        final range = math.max(60.0, maxR - startR);
        final targetSeconds = 1.7;
        final baseSpeed = (range / (targetSeconds * 60.0)) * frames;
        ripple.r += baseSpeed * (0.85 + 0.30 * energy);
        ripple.a *= math.pow(0.996, frames).toDouble();
      }

      final edgeFade = _clamp01((maxR - ripple.r) / math.max(34.0, math.min(size.width, size.height) * 0.14));
      final alpha = ripple.a * (0.35 + 0.65 * edgeFade);
      if (alpha <= 0.02 || ripple.r > maxR + 28) continue;

      final thickness = _clamp(math.min(size.width, size.height) * 0.06, 10.0, 26.0) *
          (0.62 + 0.38 * edgeFade);
      final double outer = (ripple.r + thickness * 0.5).toDouble();
      final double inner = math.max(0.0, ripple.r - thickness * 0.5).toDouble();

      final path = _buildWavyAnnulus(center, outer, inner, ripple, nowMs);
      final shader = ui.Gradient.radial(
        center,
        outer + ripple.wobble,
        <Color>[
          _hsla(hue, 0.60, 0.70, 0),
          _hsla(hue, 0.65, 0.75, alpha * 0.25),
          _hsla(hue, 0.70, 0.80, alpha),
          _hsla(hue, 0.65, 0.75, alpha * 0.25),
          _hsla(hue, 0.60, 0.70, 0),
        ],
        const <double>[0.0, 0.45, 0.5, 0.55, 1.0],
      );
      final paint = Paint()
        ..style = PaintingStyle.fill
        ..shader = shader;
      canvas.drawPath(path, paint);

      next.add(ripple);
    }
    state.ripples
      ..clear()
      ..addAll(next);
  }

  void _drawTrippyRipple(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
    double nowMs,
  ) {
    _drawDiffusionRing(canvas, size, state, frames, nowMs);

    final center = Offset(size.width * 0.5, size.height * 0.5);
    final smoothed = state.smoothFreqLinear;
    var bass = 0.0;
    for (var i = 0; i < math.min(5, smoothed.length); i++) {
      bass += smoothed[i];
    }
    bass = bass / math.max(1, math.min(5, smoothed.length));
    final ringR = math.min(size.width, size.height) * (0.28 + 0.18 * _clamp01(bass));
    final glow = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.2
      ..color = _hsla(_themeHue(smoothed), 0.95, 0.72, 0.42);
    canvas.drawCircle(center, ringR, glow);
  }

  void _drawAttachmentRing(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
    double nowMs,
  ) {
    final center = Offset(size.width * 0.5, size.height * 0.5);
    final canvasR = math.min(size.width, size.height) * 0.5;
    final coverR = canvasR * 0.70;
    final maxR = math.max(2.0, canvasR - 2.0);
    final r = math.min(maxR - 2.0, coverR + math.max(12.0, coverR * 0.08));
    final band = math.max(0.0, maxR - r);
    if (band < 6) return;

    const bars = 96;
    state.attachmentLevels ??= Float64List(bars);
    state.attachmentSpikes ??= Float64List(bars);
    final levels = state.attachmentLevels!;
    final spikes = state.attachmentSpikes!;

    final smoothed = state.smoothFreq;
    if (isPlaying && _detectBeat(smoothed) && nowMs - state.attachmentLastBeatMs > 180) {
      state.attachmentLastBeatMs = nowMs;
      state.attachmentPunch = 1.0;
      var peakIdx = 0;
      var peakVal = -1.0;
      for (var i = 0; i < bars; i++) {
        final raw = _sampleLogBand(smoothed, i / bars, 0.06, 1.0);
        if (raw > peakVal) {
          peakVal = raw;
          peakIdx = i;
        }
      }
      for (var d = -3; d <= 3; d++) {
        spikes[(peakIdx + d + bars) % bars] = 1;
      }
    } else {
      state.attachmentPunch =
          math.max(0.0, state.attachmentPunch - (frames / 60.0) * 3.2);
    }
    for (var i = 0; i < bars; i++) {
      spikes[i] *= math.pow(0.86, frames).toDouble();
    }

    state.attachmentRotation += (isPlaying ? 0.18 : 0.08) * (frames / 60.0);
    final maxLen = math.min(maxR - r, math.max(math.min(size.width, size.height) * 0.08, band * 0.70));
    final minLen = math.max(6.0, maxLen * 0.08);
    final points = <Offset>[];

    for (var i = 0; i < bars; i++) {
      final t = i / bars;
      final raw = isPlaying ? _sampleLogBand(smoothed, t, 0.06, 1.0) : 0.0;
      final gain = 0.55 + 0.95 * (1 - (2 * t - 1).abs());
      final target = (math.pow(raw, 2.05).toDouble() * gain + spikes[i] * 1.25) * (1 + 0.55 * state.attachmentPunch);
      final current = levels[i];
      levels[i] = target > current ? _lerp(current, target, 0.68) : _lerp(current, target, 0.06);
      final idle = isPlaying ? 0 : _idleNoise(nowMs, i.toDouble());
      final v = _clamp01(levels[i] + idle);
      final rr = math.min(maxR, r + minLen * 0.55 + math.pow(v, 1.15).toDouble() * math.max(10.0, maxLen));
      final a = t * math.pi * 2 + state.attachmentRotation;
      points.add(center + Offset(math.cos(a) * rr, math.sin(a) * rr));
    }
    if (points.length < 3) return;

    final hue = _themeHue(smoothed);
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3.2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..shader = ui.Gradient.sweep(
        center,
        <Color>[
          _hsla(hue, 0.90, 0.65, 0.95),
          _hsla(hue + 45, 0.90, 0.70, 0.90),
          _hsla(hue, 0.90, 0.65, 0.95),
        ],
        const <double>[0.0, 0.5, 1.0],
      );
    canvas.drawPath(_buildClosedSpline(points), stroke);

    final inner = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.2
      ..color = _hsla(hue, 0.90, 0.78, 0.65);
    canvas.drawPath(_buildClosedSpline(points), inner);
  }

  void _drawRotatingCover(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
  ) {
    final w = size.width;
    final h = size.height;
    final center = Offset(w * 0.5, h * 0.5);
    final canvasR = math.min(w, h) * 0.5;
    final coverR = canvasR * 0.70;
    final maxR = math.max(2.0, canvasR - 2.0);
    final r = math.min(maxR - 2.0, coverR + math.max(12.0, coverR * 0.08));
    final band = math.max(0.0, maxR - r);
    if (band < 2) return;

    final bars = 100;
    final smoothed = state.smoothFreq;
    final hue = _themeHue(smoothed);
    final maxLen = math.min(band, math.max(math.min(w, h) * 0.06, band * 0.55));
    final minLen = math.min(math.max(4.0, maxLen * 0.10), band);

    canvas.save();
    canvas.translate(center.dx, center.dy);
    canvas.rotate(isPlaying ? state.rotation : 0);
    for (var i = 0; i < bars; i++) {
      final t = i / bars;
      final v = isPlaying ? _sampleLogBand(smoothed, t, 0.04, 1.0) : 0.10;
      final len = minLen + _clamp01(v) * math.max(0.0, maxLen - minLen);
      final a = t * math.pi * 2;
      final p1 = Offset(math.cos(a) * r, math.sin(a) * r);
      final p2 = Offset(math.cos(a) * (r + len), math.sin(a) * (r + len));
      final paint = Paint()
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round
        ..strokeWidth = 3
        ..color = _hsla(hue, 0.90, 0.65, _clamp01(0.25 + _clamp01(v) * 0.75));
      canvas.drawLine(p1, p2, paint);
    }

    var bass = 0.0;
    for (var i = 1; i < math.min(6, smoothed.length); i++) {
      bass += smoothed[i];
    }
    final minArc = isPlaying ? 0.22 : 0.16;
    final rawEnergy = _clamp01(minArc + (bass / 2.6) * 0.78);
    state.rotatingEnergySmooth = _lerp(state.rotatingEnergySmooth, rawEnergy, 0.12);
    final targetArc = state.rotatingEnergySmooth;
    state.rotatingArcEnergy = targetArc > state.rotatingArcEnergy
        ? _lerp(state.rotatingArcEnergy, targetArc, 0.25)
        : _lerp(state.rotatingArcEnergy, targetArc, 0.06);
    final arcLen = _clamp01(state.rotatingArcEnergy) * math.pi * 2 * 0.75;
    final outerR = math.min(maxR - 2.0, r + maxLen + math.min(w, h) * 0.02);

    final base = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = math.max(1.3, math.min(w, h) * 0.006)
      ..color = _hsla(hue, 0.90, 0.75, 0.35);
    canvas.drawCircle(Offset.zero, outerR, base);

    final arc = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = math.max(2.5, math.min(w, h) * 0.015)
      ..strokeCap = StrokeCap.round
      ..color = _hsla(hue, 0.90, 0.70, 0.70);
    canvas.drawArc(
      Rect.fromCircle(center: Offset.zero, radius: outerR),
      -math.pi / 2,
      arcLen,
      false,
      arc,
    );
    canvas.restore();
  }

  void _drawBottom(
    Canvas canvas,
    Size size,
    _SpectrumRuntimeState state,
    double frames,
  ) {
    switch (mode) {
      case SpectrumMode.columnar:
        _drawBottomColumnar(canvas, size, state, frames);
        return;
      case SpectrumMode.bessel:
        _drawBottomBessel(canvas, size, state, frames);
        return;
      case SpectrumMode.wave:
      case SpectrumMode.godRing:
      case SpectrumMode.diffusionRing:
      case SpectrumMode.trippyRipple:
      case SpectrumMode.attachmentRing:
      case SpectrumMode.rotatingCover:
        _drawBottomWave(canvas, size, state, frames);
        return;
    }
  }

  void _drawBottomWave(Canvas canvas, Size size, _SpectrumRuntimeState state, double frames) {
    const barCount = 48;
    final bars = _updateBottomBars(state, barCount, frames);
    final w = size.width;
    final h = size.height;
    const gap = 2.0;
    final barWidth = (w - gap * (barCount - 1)) / barCount;
    final points = <Offset>[];
    for (var i = 0; i < barCount; i++) {
      final x = i * (barWidth + gap) + barWidth * 0.5;
      final hh = bars[i] * h * 0.9;
      points.add(Offset(x, h - hh));
    }
    if (points.length < 2) return;

    final fillPath = Path()
      ..moveTo(0, h)
      ..lineTo(points.first.dx, points.first.dy);
    _appendSmoothSegments(fillPath, points, tension: 0.35);
    fillPath
      ..lineTo(w, h)
      ..close();

    final fillPaint = Paint()
      ..style = PaintingStyle.fill
      ..shader = ui.Gradient.linear(
        const Offset(0, 0),
        Offset(0, h),
        isColorful
            ? const <Color>[
                Color.fromRGBO(59, 130, 246, 0.35),
                Color.fromRGBO(168, 85, 247, 0.22),
                Color.fromRGBO(16, 185, 129, 0.14),
                Color.fromRGBO(255, 255, 255, 0.02),
              ]
            : const <Color>[
                Color.fromRGBO(255, 255, 255, 0.25),
                Color.fromRGBO(255, 255, 255, 0.12),
                Color.fromRGBO(255, 255, 255, 0.02),
              ],
        isColorful ? const <double>[0.0, 0.35, 0.7, 1.0] : const <double>[0.0, 0.4, 1.0],
      );
    canvas.drawPath(fillPath, fillPaint);

    final topPath = Path()..moveTo(points.first.dx, points.first.dy);
    _appendSmoothSegments(topPath, points, tension: 0.35);
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5
      ..strokeJoin = StrokeJoin.round
      ..strokeCap = StrokeCap.round
      ..color = isColorful
          ? const Color.fromRGBO(147, 197, 253, 0.65)
          : const Color.fromRGBO(255, 255, 255, 0.35);
    canvas.drawPath(topPath, stroke);
  }

  void _drawBottomBessel(Canvas canvas, Size size, _SpectrumRuntimeState state, double frames) {
    final w = size.width;
    final h = size.height;
    final count = _clampInt((w / 10).floor(), 48, 96);
    final bars = _updateBottomBars(state, count, frames);
    final points = <Offset>[];
    for (var i = 0; i < count; i++) {
      final t = i / (count - 1);
      points.add(Offset(t * w, h - bars[i] * h * 0.88));
    }
    if (points.length < 2) return;

    final path = Path()
      ..moveTo(0, h)
      ..lineTo(points.first.dx, points.first.dy);
    _appendSmoothSegments(path, points, tension: 0.32);
    path
      ..lineTo(w, h)
      ..close();

    final paint = Paint()
      ..style = PaintingStyle.fill
      ..shader = ui.Gradient.linear(
        const Offset(0, 0),
        Offset(w, h),
        isColorful
            ? const <Color>[
                Color.fromRGBO(59, 130, 246, 0.35),
                Color.fromRGBO(168, 85, 247, 0.22),
                Color.fromRGBO(16, 185, 129, 0.14),
              ]
            : const <Color>[
                Color.fromRGBO(255, 255, 255, 0.25),
                Color.fromRGBO(255, 255, 255, 0.12),
                Color.fromRGBO(255, 255, 255, 0.02),
              ],
        isColorful ? const <double>[0.0, 0.5, 1.0] : const <double>[0.0, 0.55, 1.0],
      );
    canvas.drawPath(path, paint);
  }

  void _drawBottomColumnar(Canvas canvas, Size size, _SpectrumRuntimeState state, double frames) {
    final w = size.width;
    final h = size.height;
    final barCount = _clampInt((w / 5).floor(), 72, 128);
    final bars = _updateBottomBars(state, barCount, frames);
    const gap = 1.0;
    final barWidth = (w - gap * (barCount - 1)) / barCount;

    state.bottomPeaks ??= Float64List(barCount);
    if (state.bottomPeaks!.length != barCount) {
      state.bottomPeaks = Float64List(barCount)..fillRange(0, barCount, h);
    }
    final peaks = state.bottomPeaks!;

    final gradient = ui.Gradient.linear(
      const Offset(0, 0),
      Offset(w, h),
      isColorful
          ? const <Color>[
              Color.fromRGBO(59, 130, 246, 0.95),
              Color.fromRGBO(168, 85, 247, 0.85),
              Color.fromRGBO(16, 185, 129, 0.75),
            ]
          : const <Color>[
              Color.fromRGBO(255, 255, 255, 0.55),
              Color.fromRGBO(255, 255, 255, 0.28),
              Color.fromRGBO(255, 255, 255, 0.16),
            ],
      isColorful ? const <double>[0.0, 0.5, 1.0] : const <double>[0.0, 0.6, 1.0],
    );
    final barPaint = Paint()..shader = gradient;
    final peakPaint = Paint()..color = const Color.fromRGBO(255, 255, 255, 0.85);

    for (var i = 0; i < barCount; i++) {
      final barH = bars[i] * h * 0.92;
      final x = i * (barWidth + gap);
      final y = h - barH;
      canvas.drawRect(Rect.fromLTWH(x, y, barWidth, barH), barPaint);

      final blockH = math.max(2.0, barWidth * 0.32);
      final desiredY = math.max(0.0, y - blockH - 2);
      final currentPeak = peaks[i];
      peaks[i] = desiredY < currentPeak
          ? desiredY
          : math.min(h - blockH, currentPeak + 1.4 * frames);
      canvas.drawRect(Rect.fromLTWH(x, peaks[i], barWidth, blockH), peakPaint);
    }
  }

  Float64List _updateBottomBars(_SpectrumRuntimeState state, int count, double frames) {
    state.bottomLevels ??= Float64List(count);
    state.bottomVelocity ??= Float64List(count);
    if (state.bottomLevels!.length != count) {
      state.bottomLevels = Float64List(count);
    }
    if (state.bottomVelocity!.length != count) {
      state.bottomVelocity = Float64List(count);
    }

    final bars = state.bottomLevels!;
    final velocity = state.bottomVelocity!;
    final freq = state.smoothFreqLinear;
    final bands = freq.length;
    final attack = 1 - math.pow(1 - 0.6, frames).toDouble();
    final gravity = 0.008 * frames;

    for (var i = 0; i < count; i++) {
      final t = i / count;
      final freqIdx = _clampInt((t * t * bands * 0.9 + t * bands * 0.1).floor(), 0, bands - 1);
      var sum = 0.0;
      var sampleCount = 0;
      final start = _clampInt(freqIdx - 1, 0, bands - 1);
      final end = _clampInt(freqIdx + 1, 0, bands - 1);
      for (var j = start; j <= end; j++) {
        sum += freq[j];
        sampleCount++;
      }

      final raw = isPlaying ? sum / sampleCount : 0.0;
      final target = math.pow(raw, 0.8).toDouble() * 1.2;
      if (target > bars[i]) {
        bars[i] = bars[i] + (target - bars[i]) * attack;
        velocity[i] = 0;
      } else {
        velocity[i] += gravity;
        bars[i] -= velocity[i] * frames;
        if (bars[i] < target) {
          bars[i] = target;
          velocity[i] = 0;
        }
      }
      bars[i] = _clamp01(bars[i]);
    }
    return bars;
  }

  void _resampleToFixed(List<int> source, Float64List output, {required double gamma}) {
    if (source.isEmpty) {
      for (var i = 0; i < output.length; i++) {
        output[i] = 0;
      }
      return;
    }
    final last = source.length - 1;
    for (var i = 0; i < output.length; i++) {
      final idx = _clampInt(((i / (output.length - 1)) * last).floor(), 0, last);
      final normalized = _clamp01(source[idx] / 255.0);
      output[i] = gamma == 1.0 ? normalized : math.pow(normalized, gamma).toDouble();
    }
  }

  void _smooth(Float64List prev, Float64List next, double alpha) {
    for (var i = 0; i < prev.length; i++) {
      prev[i] += (next[i] - prev[i]) * alpha;
    }
  }

  double _sampleLogBand(Float64List freq, double t, double bandStart, double bandEnd) {
    if (freq.isEmpty) return 0;
    final start = _clamp01(bandStart);
    final end = _clamp01(bandEnd);
    final u = start + (end - start) * math.pow(_clamp01(t), 2.2);
    final idx = _clampInt((u * (freq.length - 1)).floor(), 0, freq.length - 1);
    return freq[idx];
  }

  bool _detectBeat(Float64List smoothedFreq) {
    var bass = 0.0;
    for (var i = 1; i < math.min(6, smoothedFreq.length); i++) {
      bass += smoothedFreq[i];
    }
    return bass > 1.6;
  }

  double _themeHue(Float64List smoothedFreq) {
    var energy = 0.0;
    for (var i = 0; i < math.min(20, smoothedFreq.length); i++) {
      energy += smoothedFreq[i];
    }
    return 200 + energy * 40;
  }

  double _energy(Float64List smoothedFreq) {
    var sum = 0.0;
    for (var i = 0; i < math.min(20, smoothedFreq.length); i++) {
      sum += smoothedFreq[i];
    }
    return _clamp01(sum / 8.0);
  }

  double _idleNoise(double nowMs, double seed) {
    final t = nowMs * 0.001;
    return 0.05 * (0.5 + 0.5 * math.sin(t * 1.1 + seed * 2.37)) +
        0.02 * (0.5 + 0.5 * math.sin(t * 0.63 + seed * 7.1));
  }

  Path _buildWavyAnnulus(Offset center, double outer, double inner, _Ripple ripple, double nowMs) {
    const seg = 120;
    final t = nowMs * 0.001;
    final outerPts = <Offset>[];
    final innerPts = <Offset>[];
    for (var i = 0; i <= seg; i++) {
      final a = (i / seg) * math.pi * 2;
      final n = 0.65 * math.sin(a * 2.5 + ripple.phase + t * 1.15) +
          0.35 * math.sin(a * 4.5 - ripple.phase * 0.7 + t * 0.85);
      final o = outer + ripple.wobble * n;
      final ii = inner + ripple.wobble * 0.6 * n;
      outerPts.add(center + Offset(math.cos(a) * o, math.sin(a) * o));
      innerPts.add(center + Offset(math.cos(a) * ii, math.sin(a) * ii));
    }

    final path = Path()..moveTo(outerPts.first.dx, outerPts.first.dy);
    for (var i = 1; i < outerPts.length; i++) {
      path.lineTo(outerPts[i].dx, outerPts[i].dy);
    }
    for (var i = innerPts.length - 1; i >= 0; i--) {
      path.lineTo(innerPts[i].dx, innerPts[i].dy);
    }
    path.close();
    return path;
  }

  void _appendSmoothSegments(Path path, List<Offset> points, {required double tension}) {
    for (var i = 0; i < points.length - 1; i++) {
      final p0 = points[i > 0 ? i - 1 : 0];
      final p1 = points[i];
      final p2 = points[i + 1];
      final p3 = points[i + 2 < points.length ? i + 2 : points.length - 1];
      final cp1 = Offset(
        p1.dx + (p2.dx - p0.dx) * tension / 3,
        p1.dy + (p2.dy - p0.dy) * tension / 3,
      );
      final cp2 = Offset(
        p2.dx - (p3.dx - p1.dx) * tension / 3,
        p2.dy - (p3.dy - p1.dy) * tension / 3,
      );
      path.cubicTo(cp1.dx, cp1.dy, cp2.dx, cp2.dy, p2.dx, p2.dy);
    }
  }

  Path _buildClosedSpline(List<Offset> points) {
    if (points.length < 3) {
      return Path();
    }
    final path = Path();
    final first = points[0];
    final second = points.length > 1 ? points[1] : first;
    path.moveTo((first.dx + second.dx) * 0.5, (first.dy + second.dy) * 0.5);
    for (var i = 1; i < points.length; i++) {
      final p = points[i];
      final n = points[(i + 1) % points.length];
      final mx = (p.dx + n.dx) * 0.5;
      final my = (p.dy + n.dy) * 0.5;
      path.quadraticBezierTo(p.dx, p.dy, mx, my);
    }
    path.close();
    return path;
  }

  Color _hsla(double h, double s, double l, double a) {
    final hue = ((h % 360) + 360) % 360;
    return HSLColor.fromAHSL(_clamp01(a), hue, _clamp01(s), _clamp01(l)).toColor();
  }

  double _clamp(double v, double min, double max) {
    if (v < min) return min;
    if (v > max) return max;
    return v;
  }

  double _clamp01(double v) => _clamp(v, 0, 1);

  double _lerp(double a, double b, double t) => a + (b - a) * _clamp01(t);

  int _clampInt(int v, int min, int max) {
    if (v < min) return min;
    if (v > max) return max;
    return v;
  }
}

class _SpectrumRuntimeState {
  final Float64List freq = Float64List(SpectrumPainter._fftSize);
  final Float64List freqLinear = Float64List(SpectrumPainter._fftSize);
  final Float64List wave = Float64List(SpectrumPainter._fftSize);
  final Float64List smoothFreq = Float64List(SpectrumPainter._fftSize);
  final Float64List smoothFreqLinear = Float64List(SpectrumPainter._fftSize);
  final Float64List smoothWave = Float64List(SpectrumPainter._fftSize);

  double lastFrameMs = 0;
  double rotation = 0;

  double pulse = 0;
  double lastBeatMs = 0;
  double lastDiffusionSpawnMs = 0;

  double rotatingArcEnergy = 0;
  double rotatingEnergySmooth = 0;

  double attachmentRotation = 0;
  double attachmentPunch = 0;
  double attachmentLastBeatMs = 0;

  Float64List? attachmentLevels;
  Float64List? attachmentSpikes;

  Float64List? bottomLevels;
  Float64List? bottomVelocity;
  Float64List? bottomPeaks;

  final List<_Ripple> ripples = <_Ripple>[];
}

class _Ripple {
  _Ripple({
    required this.r,
    required this.a,
    required this.phase,
    required this.wobble,
  });

  double r;
  double a;
  double phase;
  double wobble;
}

class _GodRingCfg {
  const _GodRingCfg({
    required this.t,
    required this.bars,
    required this.bandStart,
    required this.bandEnd,
    required this.speed,
  });

  final double t;
  final int bars;
  final double bandStart;
  final double bandEnd;
  final double speed;
}
