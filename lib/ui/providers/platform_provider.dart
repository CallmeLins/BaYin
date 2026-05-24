import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Equivalent to `src-ui/src/hooks/usePlatform.ts` — a cheap, cached snapshot
/// of the host platform, exposed as Riverpod so it can be watched from any
/// widget or provider without sprinkling `Platform.isX` checks through the
/// tree.
enum PlatformKind { windows, macos, linux, android, ios, fuchsia, unknown }

class PlatformInfo {
  const PlatformInfo({
    required this.kind,
    required this.isDesktop,
    required this.isMobile,
    required this.operatingSystem,
    required this.operatingSystemVersion,
  });

  final PlatformKind kind;
  final bool isDesktop;
  final bool isMobile;
  final String operatingSystem;
  final String operatingSystemVersion;

  bool get isWindows => kind == PlatformKind.windows;
  bool get isMacOS => kind == PlatformKind.macos;
  bool get isLinux => kind == PlatformKind.linux;
  bool get isAndroid => kind == PlatformKind.android;
  bool get isIOS => kind == PlatformKind.ios;
}

final platformProvider = Provider<PlatformInfo>((ref) {
  final PlatformKind kind;
  if (Platform.isWindows) {
    kind = PlatformKind.windows;
  } else if (Platform.isMacOS) {
    kind = PlatformKind.macos;
  } else if (Platform.isLinux) {
    kind = PlatformKind.linux;
  } else if (Platform.isAndroid) {
    kind = PlatformKind.android;
  } else if (Platform.isIOS) {
    kind = PlatformKind.ios;
  } else if (Platform.isFuchsia) {
    kind = PlatformKind.fuchsia;
  } else {
    kind = PlatformKind.unknown;
  }

  final isDesktop = kind == PlatformKind.windows ||
      kind == PlatformKind.macos ||
      kind == PlatformKind.linux;
  final isMobile = kind == PlatformKind.android || kind == PlatformKind.ios;

  return PlatformInfo(
    kind: kind,
    isDesktop: isDesktop,
    isMobile: isMobile,
    operatingSystem: Platform.operatingSystem,
    operatingSystemVersion: Platform.operatingSystemVersion,
  );
});
