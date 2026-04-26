import 'dart:convert';
import 'dart:io';

import 'package:cached_network_image/cached_network_image.dart';
import 'package:flutter/material.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

enum CoverArtSize { small, mid }

class CoverArt extends StatelessWidget {
  const CoverArt({
    super.key,
    required this.width,
    required this.height,
    this.coverHash,
    this.streamCoverUrl,
    this.streamInfo,
    this.size = CoverArtSize.small,
    this.shape = BoxShape.rectangle,
    this.borderRadius,
    this.placeholderIcon,
    this.placeholderIconSize = 16,
    this.fit = BoxFit.cover,
  });

  final double width;
  final double height;
  final String? coverHash;
  final String? streamCoverUrl;
  final String? streamInfo;
  final CoverArtSize size;
  final BoxShape shape;
  final BorderRadius? borderRadius;
  final IconData? placeholderIcon;
  final double placeholderIconSize;
  final BoxFit fit;

  @override
  Widget build(BuildContext context) {
    final network = _normalizeUrl(streamCoverUrl) ??
        _normalizeUrl(_extractCoverUrlFromStreamInfo(streamInfo));
    final localPath = _pathForCoverHash(coverHash, size);

    Widget child;
    if (network != null) {
      if (network.startsWith('data:image')) {
        child = _buildDataImage(network, context);
      } else {
        child = CachedNetworkImage(
          imageUrl: network,
          fit: fit,
          errorWidget: (context, imageUrl, error) => _placeholder(context),
          placeholder: (context, imageUrl) => _placeholder(context),
        );
      }
    } else if (localPath != null) {
      child = Image.file(
        File(localPath),
        fit: fit,
        errorBuilder: (context, error, stackTrace) => _placeholder(context),
      );
    } else {
      child = _placeholder(context);
    }

    return SizedBox(
      width: width,
      height: height,
      child: _clip(child),
    );
  }

  Widget _buildDataImage(String dataUrl, BuildContext context) {
    try {
      final comma = dataUrl.indexOf(',');
      if (comma <= 0 || comma >= dataUrl.length - 1) {
        return _placeholder(context);
      }
      final bytes = base64Decode(dataUrl.substring(comma + 1));
      return Image.memory(
        bytes,
        fit: fit,
        errorBuilder: (context, error, stackTrace) => _placeholder(context),
      );
    } catch (_) {
      return _placeholder(context);
    }
  }

  Widget _clip(Widget child) {
    if (shape == BoxShape.circle) {
      return ClipOval(child: child);
    }
    return ClipRRect(
      borderRadius: borderRadius ?? BorderRadius.circular(8),
      child: child,
    );
  }

  Widget _placeholder(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      color: scheme.surfaceContainerHighest,
      alignment: Alignment.center,
      child: Icon(
        placeholderIcon ?? PhosphorIcons.musicNote(),
        size: placeholderIconSize,
        color: scheme.onSurfaceVariant,
      ),
    );
  }
}

String? _pathForCoverHash(String? hash, CoverArtSize size) {
  if (hash == null || hash.isEmpty) {
    return null;
  }
  final prefix = hash.length >= 2 ? hash.substring(0, 2) : hash;
  final folder = size == CoverArtSize.mid ? 'mid' : 'small';
  final base = _coverBaseDir();
  return '$base${Platform.pathSeparator}$folder'
      '${Platform.pathSeparator}$prefix${Platform.pathSeparator}$hash.jpg';
}

String _coverBaseDir() {
  final sep = Platform.pathSeparator;
  late final String appData;
  if (Platform.isWindows) {
    appData = Platform.environment['APPDATA'] ?? Directory.current.path;
  } else if (Platform.isMacOS || Platform.isIOS) {
    final home = Platform.environment['HOME'] ?? Directory.current.path;
    appData = '$home${sep}Library${sep}Application Support';
  } else if (Platform.isLinux || Platform.isAndroid) {
    final xdg = Platform.environment['XDG_DATA_HOME'];
    if (xdg != null && xdg.isNotEmpty) {
      appData = xdg;
    } else {
      final home = Platform.environment['HOME'] ?? Directory.current.path;
      appData = '$home$sep.local${sep}share';
    }
  } else {
    appData = Directory.current.path;
  }
  return '$appData${sep}BaYin${sep}covers';
}

String? _extractCoverUrlFromStreamInfo(String? raw) {
  if (raw == null || raw.isEmpty) {
    return null;
  }
  if (!raw.trimLeft().startsWith('{')) {
    return null;
  }
  try {
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      return null;
    }
    final value = decoded['coverUrl'];
    if (value is String && value.trim().isNotEmpty) {
      return value.trim();
    }
    return null;
  } catch (_) {
    return null;
  }
}

String? _normalizeUrl(String? value) {
  if (value == null) {
    return null;
  }
  final trimmed = value.trim();
  if (trimmed.isEmpty) {
    return null;
  }
  return trimmed;
}
