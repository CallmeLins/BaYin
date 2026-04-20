import '../rust/rust_api.dart';

class ScannedSong {
  const ScannedSong({
    required this.id,
    required this.title,
    required this.artist,
    required this.album,
    required this.duration,
    required this.filePath,
    required this.fileSize,
    this.coverUrl,
    this.isHr,
    this.isSq,
    this.format,
    this.bitDepth,
    this.sampleRate,
    this.bitrate,
    this.channels,
    this.createdAt,
  });

  factory ScannedSong.fromRust(RustScannedSong value) {
    return ScannedSong(
      id: value.id,
      title: value.title,
      artist: value.artist,
      album: value.album,
      duration: value.duration,
      filePath: value.filePath,
      fileSize: value.fileSize,
      coverUrl: value.coverUrl,
      isHr: value.isHr,
      isSq: value.isSq,
      format: value.format,
      bitDepth: value.bitDepth,
      sampleRate: value.sampleRate,
      bitrate: value.bitrate,
      channels: value.channels,
      createdAt: value.createdAt,
    );
  }

  final String id;
  final String title;
  final String artist;
  final String album;
  final double duration;
  final String filePath;
  final int fileSize;
  final String? coverUrl;
  final bool? isHr;
  final bool? isSq;
  final String? format;
  final int? bitDepth;
  final int? sampleRate;
  final int? bitrate;
  final int? channels;
  final int? createdAt;
}
