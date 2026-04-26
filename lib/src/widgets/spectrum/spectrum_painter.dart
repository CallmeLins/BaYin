import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../models/models.dart';

class SpectrumPainter extends CustomPainter {
  SpectrumPainter({
    required this.frequency,
    required this.waveform,
    required this.mode,
    required this.color,
  });

  final List<int> frequency;
  final List<int> waveform;
  final SpectrumMode mode;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round
      ..strokeWidth = 2
      ..color = color;

    switch (mode) {
      case SpectrumMode.wave:
        _paintWave(canvas, size, paint);
      case SpectrumMode.godRing:
        _paintGodRing(canvas, size, paint);
      case SpectrumMode.diffusionRing:
        _paintDiffusionRing(canvas, size, paint);
      case SpectrumMode.trippyRipple:
        _paintTrippyRipple(canvas, size, paint);
      case SpectrumMode.attachmentRing:
        _paintAttachmentRing(canvas, size, paint);
      case SpectrumMode.rotatingCover:
        _paintRotatingCover(canvas, size, paint);
      case SpectrumMode.bessel:
        _paintBessel(canvas, size, paint);
      case SpectrumMode.columnar:
        _paintColumnar(canvas, size, paint);
    }
  }

  @override
  bool shouldRepaint(covariant SpectrumPainter oldDelegate) {
    return oldDelegate.mode != mode ||
        oldDelegate.frequency != frequency ||
        oldDelegate.waveform != waveform ||
        oldDelegate.color != color;
  }

  void _paintWave(Canvas canvas, Size size, Paint paint) {
    final points = waveform.isEmpty ? _fallback(128, 128) : waveform;
    final path = Path();
    for (var i = 0; i < points.length; i++) {
      final x = i / (points.length - 1) * size.width;
      final normalized = points[i] / 255.0;
      final y = size.height * (1 - normalized);
      if (i == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }
    canvas.drawPath(path, paint);
  }

  void _paintGodRing(Canvas canvas, Size size, Paint paint) {
    final values = frequency.isEmpty ? _fallback(64, 0) : frequency;
    final center = size.center(Offset.zero);
    final baseRadius = math.min(size.width, size.height) * 0.23;
    for (var i = 0; i < values.length; i++) {
      final angle = i / values.length * math.pi * 2;
      final level = values[i] / 255.0;
      final outer = baseRadius + level * baseRadius * 0.9;
      final p1 = Offset(
        center.dx + math.cos(angle) * baseRadius,
        center.dy + math.sin(angle) * baseRadius,
      );
      final p2 = Offset(
        center.dx + math.cos(angle) * outer,
        center.dy + math.sin(angle) * outer,
      );
      canvas.drawLine(p1, p2, paint);
    }
  }

  void _paintDiffusionRing(Canvas canvas, Size size, Paint paint) {
    final values = frequency.isEmpty ? _fallback(64, 0) : frequency;
    final center = size.center(Offset.zero);
    final energy = values.fold<double>(0, (sum, value) => sum + value) / values.length / 255.0;
    final baseRadius = math.min(size.width, size.height) * 0.2;
    for (var i = 0; i < 4; i++) {
      final radius = baseRadius + i * 14 + energy * 26;
      canvas.drawCircle(
        center,
        radius,
        paint
          ..strokeWidth = 1.5
          ..color = color.withValues(alpha: (0.7 - i * 0.15).clamp(0.15, 0.7)),
      );
    }
  }

  void _paintTrippyRipple(Canvas canvas, Size size, Paint paint) {
    final values = waveform.isEmpty ? _fallback(128, 128) : waveform;
    final center = size.center(Offset.zero);
    final baseRadius = math.min(size.width, size.height) * 0.17;
    for (var layer = 0; layer < 3; layer++) {
      final path = Path();
      for (var i = 0; i < values.length; i++) {
        final angle = i / values.length * math.pi * 2;
        final wave = (values[i] / 255.0 - 0.5) * 2;
        final radius = baseRadius + layer * 18 + wave * (8 + layer * 3);
        final p = Offset(
          center.dx + math.cos(angle) * radius,
          center.dy + math.sin(angle) * radius,
        );
        if (i == 0) {
          path.moveTo(p.dx, p.dy);
        } else {
          path.lineTo(p.dx, p.dy);
        }
      }
      path.close();
      canvas.drawPath(
        path,
        paint
          ..strokeWidth = 1.4
          ..color = color.withValues(alpha: (0.8 - layer * 0.2).clamp(0.2, 0.8)),
      );
    }
  }

  void _paintAttachmentRing(Canvas canvas, Size size, Paint paint) {
    final values = frequency.isEmpty ? _fallback(64, 0) : frequency;
    final center = size.center(Offset.zero);
    final radius = math.min(size.width, size.height) * 0.26;
    canvas.drawCircle(center, radius, paint..strokeWidth = 1.2);

    for (var i = 0; i < values.length; i++) {
      final angle = i / values.length * math.pi * 2;
      final level = values[i] / 255.0;
      final p1 = Offset(
        center.dx + math.cos(angle) * radius,
        center.dy + math.sin(angle) * radius,
      );
      final p2 = Offset(
        center.dx + math.cos(angle) * (radius + 6 + level * 28),
        center.dy + math.sin(angle) * (radius + 6 + level * 28),
      );
      canvas.drawLine(p1, p2, paint..strokeWidth = 2);
    }
  }

  void _paintRotatingCover(Canvas canvas, Size size, Paint paint) {
    final values = frequency.isEmpty ? _fallback(64, 0) : frequency;
    final center = size.center(Offset.zero);
    final baseRadius = math.min(size.width, size.height) * 0.24;
    final offset = values.fold<int>(0, (a, b) => a + b) % 360;
    final startAngle = offset / 180 * math.pi;

    for (var i = 0; i < 5; i++) {
      final level = values[(i * 7) % values.length] / 255.0;
      final radius = baseRadius + i * 10;
      canvas.drawArc(
        Rect.fromCircle(center: center, radius: radius),
        startAngle + i * 0.4,
        math.pi * (0.6 + level * 0.8),
        false,
        paint
          ..strokeWidth = 2 + level * 2
          ..color = color.withValues(alpha: (0.75 - i * 0.1).clamp(0.2, 0.75)),
      );
    }
  }

  void _paintBessel(Canvas canvas, Size size, Paint paint) {
    final values = waveform.isEmpty ? _fallback(128, 128) : waveform;
    final center = size.center(Offset.zero);
    final path = Path();
    for (var i = 0; i < values.length; i++) {
      final t = i / values.length * math.pi * 2;
      final wave = (values[i] / 255.0 - 0.5) * 2;
      final x = center.dx + math.cos(t * 2.3) * (size.width * 0.24 + wave * 22);
      final y = center.dy + math.sin(t * 3.1) * (size.height * 0.2 + wave * 18);
      if (i == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }
    path.close();
    canvas.drawPath(
      path,
      paint
        ..strokeWidth = 1.8
        ..color = color.withValues(alpha: 0.85),
    );
  }

  void _paintColumnar(Canvas canvas, Size size, Paint paint) {
    final values = frequency.isEmpty ? _fallback(48, 0) : frequency;
    final bars = values.length.clamp(12, 64);
    final barWidth = size.width / bars;
    for (var i = 0; i < bars; i++) {
      final level = values[i] / 255.0;
      final h = size.height * (0.1 + level * 0.9);
      final left = i * barWidth + barWidth * 0.1;
      final rect = RRect.fromRectAndRadius(
        Rect.fromLTWH(left, size.height - h, barWidth * 0.8, h),
        const Radius.circular(4),
      );
      canvas.drawRRect(
        rect,
        Paint()
          ..style = PaintingStyle.fill
          ..color = color.withValues(alpha: (0.35 + level * 0.65).clamp(0.35, 1.0)),
      );
    }
  }

  List<int> _fallback(int size, int value) {
    return List<int>.filled(size, value, growable: false);
  }
}
