import '../rust/rust_api.dart';

class Album {
  const Album({
    required this.id,
    required this.name,
    required this.artist,
    required this.songCount,
    this.coverHash,
    this.streamCoverUrl,
  });

  factory Album.fromRust(RustDbAlbum value) {
    return Album(
      id: value.id,
      name: value.name,
      artist: value.artist,
      songCount: value.songCount,
      coverHash: value.coverHash,
      streamCoverUrl: value.streamCoverUrl,
    );
  }

  final String id;
  final String name;
  final String artist;
  final String? coverHash;
  final String? streamCoverUrl;
  final int songCount;
}
