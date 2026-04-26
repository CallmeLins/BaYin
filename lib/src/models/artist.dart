import '../rust/rust_api.dart';

class Artist {
  const Artist({
    required this.id,
    required this.name,
    required this.songCount,
    this.coverHash,
    this.streamCoverUrl,
  });

  factory Artist.fromRust(RustDbArtist value) {
    return Artist(
      id: value.id,
      name: value.name,
      songCount: value.songCount,
      coverHash: value.coverHash,
      streamCoverUrl: value.streamCoverUrl,
    );
  }

  final String id;
  final String name;
  final String? coverHash;
  final String? streamCoverUrl;
  final int songCount;
}
