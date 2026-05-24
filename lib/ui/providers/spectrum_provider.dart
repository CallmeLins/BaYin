import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../rust/rust_api.dart';

final spectrumModeProvider = StateProvider<SpectrumMode>((ref) {
  return SpectrumMode.wave;
});

final spectrumFrameProvider =
    NotifierProvider<SpectrumController, RustFftSnapshot>(
      SpectrumController.new,
    );

class SpectrumController extends Notifier<RustFftSnapshot> {
  Timer? _timer;

  @override
  RustFftSnapshot build() {
    _timer ??= Timer.periodic(
      const Duration(milliseconds: 40),
      (_) => _poll(),
    );
    ref.onDispose(() {
      _timer?.cancel();
      _timer = null;
    });
    _poll();
    return const RustFftSnapshot(
      frequency: <int>[],
      waveform: <int>[],
    );
  }

  void _poll() {
    try {
      state = RustApi.instance.getAudioFft();
    } catch (_) {
      // Keep last frame when FFT isn't available.
    }
  }
}
