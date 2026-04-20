import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../rust/rust_api.dart';
import '../services/settings_service.dart';
import 'settings_provider.dart';

class AppSettingsState {
  const AppSettingsState({
    required this.showCoverInList,
    required this.visualizerEnabled,
    required this.bassEffectEnabled,
    required this.proEnabled,
    required this.proGlassEnabled,
    required this.proColorSpectrumEnabled,
    required this.proPureModeEnabled,
    required this.pureModeEnabled,
    required this.lyricsFontSize,
    required this.lyricsTranslationFontSize,
    required this.lyricsOffsetMs,
    required this.lyricsPosition,
    required this.lyricsSelectable,
    required this.lyricsWordByWordAnimation,
    required this.lyricsAutoBlur,
    required this.eqEnabled,
    required this.eqPreset,
    required this.eqGains,
  });

  factory AppSettingsState.defaults() {
    return const AppSettingsState(
      showCoverInList: true,
      visualizerEnabled: false,
      bassEffectEnabled: false,
      proEnabled: false,
      proGlassEnabled: false,
      proColorSpectrumEnabled: false,
      proPureModeEnabled: false,
      pureModeEnabled: false,
      lyricsFontSize: 20,
      lyricsTranslationFontSize: 17,
      lyricsOffsetMs: 0,
      lyricsPosition: 'left',
      lyricsSelectable: false,
      lyricsWordByWordAnimation: true,
      lyricsAutoBlur: true,
      eqEnabled: false,
      eqPreset: 'balance',
      eqGains: <double>[0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
  }

  final bool showCoverInList;
  final bool visualizerEnabled;
  final bool bassEffectEnabled;
  final bool proEnabled;
  final bool proGlassEnabled;
  final bool proColorSpectrumEnabled;
  final bool proPureModeEnabled;
  final bool pureModeEnabled;
  final double lyricsFontSize;
  final double lyricsTranslationFontSize;
  final int lyricsOffsetMs;
  final String lyricsPosition;
  final bool lyricsSelectable;
  final bool lyricsWordByWordAnimation;
  final bool lyricsAutoBlur;
  final bool eqEnabled;
  final String eqPreset;
  final List<double> eqGains;

  AppSettingsState copyWith({
    bool? showCoverInList,
    bool? visualizerEnabled,
    bool? bassEffectEnabled,
    bool? proEnabled,
    bool? proGlassEnabled,
    bool? proColorSpectrumEnabled,
    bool? proPureModeEnabled,
    bool? pureModeEnabled,
    double? lyricsFontSize,
    double? lyricsTranslationFontSize,
    int? lyricsOffsetMs,
    String? lyricsPosition,
    bool? lyricsSelectable,
    bool? lyricsWordByWordAnimation,
    bool? lyricsAutoBlur,
    bool? eqEnabled,
    String? eqPreset,
    List<double>? eqGains,
  }) {
    return AppSettingsState(
      showCoverInList: showCoverInList ?? this.showCoverInList,
      visualizerEnabled: visualizerEnabled ?? this.visualizerEnabled,
      bassEffectEnabled: bassEffectEnabled ?? this.bassEffectEnabled,
      proEnabled: proEnabled ?? this.proEnabled,
      proGlassEnabled: proGlassEnabled ?? this.proGlassEnabled,
      proColorSpectrumEnabled:
          proColorSpectrumEnabled ?? this.proColorSpectrumEnabled,
      proPureModeEnabled: proPureModeEnabled ?? this.proPureModeEnabled,
      pureModeEnabled: pureModeEnabled ?? this.pureModeEnabled,
      lyricsFontSize: lyricsFontSize ?? this.lyricsFontSize,
      lyricsTranslationFontSize:
          lyricsTranslationFontSize ?? this.lyricsTranslationFontSize,
      lyricsOffsetMs: lyricsOffsetMs ?? this.lyricsOffsetMs,
      lyricsPosition: lyricsPosition ?? this.lyricsPosition,
      lyricsSelectable: lyricsSelectable ?? this.lyricsSelectable,
      lyricsWordByWordAnimation:
          lyricsWordByWordAnimation ?? this.lyricsWordByWordAnimation,
      lyricsAutoBlur: lyricsAutoBlur ?? this.lyricsAutoBlur,
      eqEnabled: eqEnabled ?? this.eqEnabled,
      eqPreset: eqPreset ?? this.eqPreset,
      eqGains: eqGains ?? this.eqGains,
    );
  }
}

class AppSettingsController extends Notifier<AppSettingsState> {
  @override
  AppSettingsState build() {
    final service = ref.watch(settingsServiceProvider);
    final defaults = AppSettingsState.defaults();
    final resolved = defaults.copyWith(
      showCoverInList:
          service.readBool(SettingsService.keyShowCoverInList) ??
          defaults.showCoverInList,
      visualizerEnabled:
          service.readBool(SettingsService.keyVisualizerEnabled) ??
          defaults.visualizerEnabled,
      bassEffectEnabled:
          service.readBool(SettingsService.keyBassEffectEnabled) ??
          defaults.bassEffectEnabled,
      proEnabled:
          service.readBool(SettingsService.keyProEnabled) ?? defaults.proEnabled,
      proGlassEnabled:
          service.readBool(SettingsService.keyProGlassEnabled) ??
          defaults.proGlassEnabled,
      proColorSpectrumEnabled:
          service.readBool(SettingsService.keyProColorSpectrumEnabled) ??
          defaults.proColorSpectrumEnabled,
      proPureModeEnabled:
          service.readBool(SettingsService.keyProPureModeEnabled) ??
          defaults.proPureModeEnabled,
      pureModeEnabled:
          service.readBool(SettingsService.keyPureModeEnabled) ??
          defaults.pureModeEnabled,
      lyricsFontSize:
          service.readDouble(SettingsService.keyLyricsFontSize) ??
          defaults.lyricsFontSize,
      lyricsTranslationFontSize:
          service.readDouble(SettingsService.keyLyricsTranslationFontSize) ??
          defaults.lyricsTranslationFontSize,
      lyricsOffsetMs:
          service.readInt(SettingsService.keyLyricsOffsetMs) ??
          defaults.lyricsOffsetMs,
      lyricsPosition:
          service.readString(SettingsService.keyLyricsPosition) ??
          defaults.lyricsPosition,
      lyricsSelectable:
          service.readBool(SettingsService.keyLyricsSelectable) ??
          defaults.lyricsSelectable,
      lyricsWordByWordAnimation:
          service.readBool(SettingsService.keyLyricsWordByWordAnimation) ??
          defaults.lyricsWordByWordAnimation,
      lyricsAutoBlur:
          service.readBool(SettingsService.keyLyricsAutoBlur) ??
          defaults.lyricsAutoBlur,
      eqEnabled:
          service.readBool(SettingsService.keyEqEnabled) ?? defaults.eqEnabled,
      eqPreset:
          service.readString(SettingsService.keyEqPreset) ?? defaults.eqPreset,
      eqGains: _readEqGains(service) ?? defaults.eqGains,
    );
    _syncEqToRust(resolved);
    return resolved;
  }

  void _syncEqToRust(AppSettingsState value) {
    try {
      RustApi.instance.audioSetEqEnabled(value.eqEnabled);
      RustApi.instance.audioSetEqGains(value.eqGains);
    } catch (_) {
      // Rust engine might be unavailable during early startup. Keep local state.
    }
  }

  List<double>? _readEqGains(SettingsService service) {
    final raw = service.readString(SettingsService.keyEqGains);
    if (raw == null || raw.isEmpty) {
      return null;
    }
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List<dynamic> || decoded.length != 10) {
        return null;
      }
      return decoded
          .map((item) {
            final value = (item as num).toDouble();
            return value.clamp(-12.0, 12.0).toDouble();
          })
          .toList(growable: false);
    } catch (_) {
      return null;
    }
  }

  Future<void> _writeEqGains(List<double> values) {
    return ref
        .read(settingsServiceProvider)
        .writeString(SettingsService.keyEqGains, jsonEncode(values));
  }

  Future<void> setShowCoverInList(bool value) async {
    state = state.copyWith(showCoverInList: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyShowCoverInList, value);
  }

  Future<void> setVisualizerEnabled(bool value) async {
    state = state.copyWith(visualizerEnabled: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyVisualizerEnabled, value);
  }

  Future<void> setBassEffectEnabled(bool value) async {
    state = state.copyWith(bassEffectEnabled: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyBassEffectEnabled, value);
  }

  Future<void> setProEnabled(bool value) async {
    state = state.copyWith(
      proEnabled: value,
      pureModeEnabled: value ? state.pureModeEnabled : false,
    );
    final service = ref.read(settingsServiceProvider);
    await service.writeBool(SettingsService.keyProEnabled, value);
    if (!value) {
      await service.writeBool(SettingsService.keyPureModeEnabled, false);
    }
  }

  Future<void> setProGlassEnabled(bool value) async {
    state = state.copyWith(proGlassEnabled: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyProGlassEnabled, value);
  }

  Future<void> setProColorSpectrumEnabled(bool value) async {
    state = state.copyWith(proColorSpectrumEnabled: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyProColorSpectrumEnabled, value);
  }

  Future<void> setProPureModeFeatureEnabled(bool value) async {
    state = state.copyWith(
      proPureModeEnabled: value,
      pureModeEnabled: value ? state.pureModeEnabled : false,
    );
    final service = ref.read(settingsServiceProvider);
    await service.writeBool(SettingsService.keyProPureModeEnabled, value);
    if (!value) {
      await service.writeBool(SettingsService.keyPureModeEnabled, false);
    }
  }

  Future<void> setPureModeEnabled(bool value) async {
    final allow = state.proEnabled && state.proPureModeEnabled;
    final next = allow ? value : false;
    state = state.copyWith(pureModeEnabled: next);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyPureModeEnabled, next);
  }

  Future<void> setLyricsFontSize(double value) async {
    final next = value.clamp(12.0, 36.0).toDouble();
    state = state.copyWith(lyricsFontSize: next);
    await ref
        .read(settingsServiceProvider)
        .writeDouble(SettingsService.keyLyricsFontSize, next);
  }

  Future<void> setLyricsTranslationFontSize(double value) async {
    final next = value.clamp(10.0, 30.0).toDouble();
    state = state.copyWith(lyricsTranslationFontSize: next);
    await ref
        .read(settingsServiceProvider)
        .writeDouble(SettingsService.keyLyricsTranslationFontSize, next);
  }

  Future<void> setLyricsOffsetMs(int value) async {
    final next = value.clamp(-3000, 3000).toInt();
    state = state.copyWith(lyricsOffsetMs: next);
    await ref
        .read(settingsServiceProvider)
        .writeInt(SettingsService.keyLyricsOffsetMs, next);
  }

  Future<void> setLyricsPosition(String value) async {
    final normalized = switch (value) {
      'left' => 'left',
      'center' => 'center',
      'right' => 'right',
      _ => 'left',
    };
    state = state.copyWith(lyricsPosition: normalized);
    await ref
        .read(settingsServiceProvider)
        .writeString(SettingsService.keyLyricsPosition, normalized);
  }

  Future<void> setLyricsSelectable(bool value) async {
    state = state.copyWith(lyricsSelectable: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyLyricsSelectable, value);
  }

  Future<void> setLyricsWordByWordAnimation(bool value) async {
    state = state.copyWith(lyricsWordByWordAnimation: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyLyricsWordByWordAnimation, value);
  }

  Future<void> setLyricsAutoBlur(bool value) async {
    state = state.copyWith(lyricsAutoBlur: value);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyLyricsAutoBlur, value);
  }

  Future<void> setEqEnabled(bool value) async {
    state = state.copyWith(eqEnabled: value);
    _syncEqToRust(state);
    await ref
        .read(settingsServiceProvider)
        .writeBool(SettingsService.keyEqEnabled, value);
  }

  Future<void> setEqPreset(String value) async {
    state = state.copyWith(eqPreset: value);
    await ref.read(settingsServiceProvider).writeString(
          SettingsService.keyEqPreset,
          value,
        );
  }

  Future<void> setEqBandGain(int index, double value) async {
    if (index < 0 || index >= state.eqGains.length) {
      return;
    }
    final next = List<double>.from(state.eqGains);
    next[index] = value.clamp(-12.0, 12.0).toDouble();
    state = state.copyWith(eqGains: List<double>.unmodifiable(next));
    _syncEqToRust(state);
    await _writeEqGains(next);
  }

  Future<void> resetEq() async {
    const next = <double>[0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    state = state.copyWith(eqGains: next, eqPreset: 'balance');
    _syncEqToRust(state);
    final service = ref.read(settingsServiceProvider);
    await service.writeString(SettingsService.keyEqPreset, 'balance');
    await _writeEqGains(next);
  }
}

final appSettingsProvider =
    NotifierProvider<AppSettingsController, AppSettingsState>(
      AppSettingsController.new,
    );
