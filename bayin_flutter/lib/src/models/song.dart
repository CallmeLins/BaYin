import '../rust/rust_api.dart';

class Song {
  const Song({
    required this.id,
    required this.title,
    required this.artist,
    required this.album,
    required this.duration,
    required this.filePath,
    required this.fileSize,
    required this.sourceType,
    required this.createdAt,
    required this.updatedAt,
    this.isHr,
    this.isSq,
    this.coverHash,
    this.serverId,
    this.serverSongId,
    this.streamInfo,
    this.fileModified,
    this.format,
    this.bitDepth,
    this.sampleRate,
    this.bitrate,
    this.channels,
  });

  factory Song.fromRust(RustDbSong value) {
    return Song(
      id: value.id,
      title: value.title,
      artist: value.artist,
      album: value.album,
      duration: value.duration,
      filePath: value.filePath,
      fileSize: value.fileSize,
      sourceType: value.sourceType,
      createdAt: value.createdAt,
      updatedAt: value.updatedAt,
      isHr: value.isHr,
      isSq: value.isSq,
      coverHash: value.coverHash,
      serverId: value.serverId,
      serverSongId: value.serverSongId,
      streamInfo: value.streamInfo,
      fileModified: value.fileModified,
      format: value.format,
      bitDepth: value.bitDepth,
      sampleRate: value.sampleRate,
      bitrate: value.bitrate,
      channels: value.channels,
    );
  }

  final String id;
  final String title;
  final String artist;
  final String album;
  final double duration;
  final String filePath;
  final int fileSize;
  final bool? isHr;
  final bool? isSq;
  final String? coverHash;
  final String sourceType;
  final String? serverId;
  final String? serverSongId;
  final String? streamInfo;
  final int? fileModified;
  final String? format;
  final int? bitDepth;
  final int? sampleRate;
  final int? bitrate;
  final int? channels;
  final int createdAt;
  final int updatedAt;
}
