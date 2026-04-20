import '../rust/rust_api.dart';

/// Local user playlist (TODO phase 3 — matches src-ui/src/types/Playlist.ts when
/// that lands). For now we only carry the fields the cached stream-playlist
/// row exposes, so a single type covers both local and stream sources by
/// convention. Phase 3 will split if needed.
class Playlist {
  const Playlist({
    required this.id,
    required this.name,
    required this.songCount,
    required this.source,
    this.serverId,
    this.kind,
    this.updatedAt,
    this.syncedAt,
  });

  factory Playlist.fromStreamRust(RustDbStreamPlaylist value) {
    return Playlist(
      id: value.playlistId,
      name: value.name,
      songCount: value.songCount,
      source: PlaylistSource.stream,
      serverId: value.serverId,
      kind: value.kind,
      updatedAt: value.updatedAt,
      syncedAt: value.syncedAt,
    );
  }

  final String id;
  final String name;
  final int songCount;
  final PlaylistSource source;
  final String? serverId;
  final String? kind;
  final int? updatedAt;
  final int? syncedAt;
}

enum PlaylistSource { local, stream }
