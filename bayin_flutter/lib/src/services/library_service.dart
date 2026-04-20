import 'dart:io';

import '../models/models.dart';
import '../rust/rust_api.dart';

class LibraryService {
  LibraryService._();

  static final LibraryService instance = LibraryService._();
  Future<String>? _databaseInitialization;

  Future<String> ensureDatabaseInitialized() async {
    return _databaseInitialization ??= _initializeDatabase();
  }

  Future<String> _initializeDatabase() async {
    final dbPath = _defaultDatabasePath();
    final dbFile = File(dbPath);
    await dbFile.parent.create(recursive: true);
    RustApi.instance.initDb(dbPath);
    return dbPath;
  }

  Future<List<Song>> loadSongs() async {
    final songs = RustApi.instance.getAllSongs();
    return songs.map(Song.fromRust).toList();
  }

  Future<List<Album>> loadAlbums() async {
    final albums = RustApi.instance.getAllAlbums();
    return albums.map(Album.fromRust).toList();
  }

  Future<List<Artist>> loadArtists() async {
    final artists = RustApi.instance.getAllArtists();
    return artists.map(Artist.fromRust).toList();
  }

  Future<List<StreamServer>> loadStreamServers() async {
    final servers = RustApi.instance.getStreamServers();
    return servers.map(StreamServer.fromRust).toList();
  }

  Future<List<Playlist>> loadStreamPlaylists(String serverId) async {
    final playlists = RustApi.instance.getStreamPlaylists(serverId);
    return playlists.map(Playlist.fromStreamRust).toList();
  }

  String _defaultDatabasePath() {
    final separator = Platform.pathSeparator;
    late final String baseDir;

    if (Platform.isWindows) {
      baseDir = Platform.environment['APPDATA'] ?? Directory.current.path;
    } else if (Platform.isMacOS || Platform.isIOS) {
      final home = Platform.environment['HOME'] ?? Directory.current.path;
      baseDir = '$home${separator}Library${separator}Application Support';
    } else if (Platform.isLinux || Platform.isAndroid) {
      final xdgDataHome = Platform.environment['XDG_DATA_HOME'];
      if (xdgDataHome != null && xdgDataHome.isNotEmpty) {
        baseDir = xdgDataHome;
      } else {
        final home = Platform.environment['HOME'] ?? Directory.current.path;
        baseDir = '$home$separator.local${separator}share';
      }
    } else {
      baseDir = Directory.current.path;
    }

    return '$baseDir${separator}BaYin${separator}bayin.db';
  }
}
