import 'dart:io';
import 'dart:isolate';

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
    await Isolate.run(() => RustApi.instance.initDb(dbPath));
    return dbPath;
  }

  Future<List<Song>> loadSongs() async {
    final songs = await Isolate.run(() => RustApi.instance.getAllSongs());
    return songs.map(Song.fromRust).toList();
  }

  Future<List<Album>> loadAlbums() async {
    final albums = await Isolate.run(() => RustApi.instance.getAllAlbums());
    return albums.map(Album.fromRust).toList();
  }

  Future<List<Artist>> loadArtists() async {
    final artists = await Isolate.run(() => RustApi.instance.getAllArtists());
    return artists.map(Artist.fromRust).toList();
  }

  Future<List<StreamServer>> loadStreamServers() async {
    final servers = await Isolate.run(() => RustApi.instance.getStreamServers());
    return servers.map(StreamServer.fromRust).toList();
  }

  Future<List<Playlist>> loadStreamPlaylists(String serverId) async {
    final playlists = await Isolate.run(() => RustApi.instance.getStreamPlaylists(serverId));
    return playlists.map(Playlist.fromStreamRust).toList();
  }

  Future<String> saveStreamServer({
    required String serverType,
    required String serverName,
    required String serverUrl,
    required String username,
    required String password,
    String? accessToken,
    String? userId,
  }) async {
    await ensureDatabaseInitialized();
    final input = RustStreamServerInput(
      serverType: serverType,
      serverName: serverName,
      serverUrl: serverUrl,
      username: username,
      password: password,
      accessToken: accessToken,
      userId: userId,
    );
    return await Isolate.run(() => RustApi.instance.saveStreamServer(input));
  }

  Future<void> deleteStreamServer(String serverId) async {
    await ensureDatabaseInitialized();
    await Isolate.run(() => RustApi.instance.deleteStreamServer(serverId));
  }

  Future<StreamConnectionTestResult> testStreamConnection({
    required String serverType,
    required String serverName,
    required String serverUrl,
    required String username,
    required String password,
    String? accessToken,
    String? userId,
  }) async {
    await ensureDatabaseInitialized();
    final input = RustStreamServerInput(
      serverType: serverType,
      serverName: serverName,
      serverUrl: serverUrl,
      username: username,
      password: password,
      accessToken: accessToken,
      userId: userId,
    );
    final result = await Isolate.run(() => RustApi.instance.testStreamConnection(input));
    return StreamConnectionTestResult(
      success: result.success,
      message: result.message,
      serverVersion: result.serverVersion,
      accessToken: result.accessToken,
      userId: result.userId,
    );
  }

  Future<RustStreamPlaylistSyncResult> syncStreamPlaylists(String serverId) async {
    await ensureDatabaseInitialized();
    return await Isolate.run(() => RustApi.instance.syncStreamPlaylists(serverId));
  }

  Future<List<Song>> loadStreamPlaylistSongs(
    String serverId,
    String playlistId,
  ) async {
    await ensureDatabaseInitialized();
    final songs = await Isolate.run(() => RustApi.instance.getStreamPlaylistSongs(
      serverId: serverId,
      playlistId: playlistId,
    ));
    return songs.map(Song.fromRust).toList();
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

class StreamConnectionTestResult {
  const StreamConnectionTestResult({
    required this.success,
    required this.message,
    this.serverVersion,
    this.accessToken,
    this.userId,
  });

  final bool success;
  final String message;
  final String? serverVersion;
  final String? accessToken;
  final String? userId;
}
