import 'dart:ffi' as ffi;
import 'dart:convert';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef _NativePing = ffi.Pointer<Utf8> Function();
typedef _NativeLastError = ffi.Pointer<Utf8> Function();
typedef _NativeStringFreeNative = ffi.Void Function(ffi.Pointer<Utf8>);
typedef _NativeStringFreeDart = void Function(ffi.Pointer<Utf8>);
typedef _NativeBoolStringArg = ffi.Bool Function(ffi.Pointer<Utf8>);
typedef _DartBoolStringArg = bool Function(ffi.Pointer<Utf8>);
typedef _NativeStringStringArg = ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>);
typedef _DartStringStringArg = ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>);
typedef _NativeBoolNoArg = ffi.Bool Function();
typedef _DartBoolNoArg = bool Function();
typedef _NativeBoolDoubleArg = ffi.Bool Function(ffi.Double);
typedef _DartBoolDoubleArg = bool Function(double);

class RustApi {
  RustApi._();

  static final RustApi instance = RustApi._();

  _RustBindings? _bindings;

  Future<void> ensureInitialized() async {
    _bindings ??= _RustBindings(_openLibrary());
  }

  String ping() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.ping();
  }

  void initDb(String dbPath) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.initDb(dbPath);
  }

  List<RustDirectoryEntry> listDirectories(String path) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.listDirectories(path);
  }

  List<RustScannedSong> scanMusicFiles(RustScanOptions options) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.scanMusicFiles(options);
  }

  RustScanAndSaveResult scanAndSaveMusicFiles(RustScanOptions options) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.scanAndSaveMusicFiles(options);
  }

  void clearAllSongs() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.clearAllSongs();
  }

  void saveScanConfig(RustScanConfig config) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.saveScanConfig(config);
  }

  RustScanConfig? getScanConfig() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getScanConfig();
  }

  RustScannedSong? getMusicMetadata(String filePath) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getMusicMetadata(filePath);
  }

  String? getLyrics(String filePath) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getLyrics(filePath);
  }

  List<RustDbSong> getAllSongs() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getAllSongs();
  }

  List<RustDbAlbum> getAllAlbums() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getAllAlbums();
  }

  List<RustDbArtist> getAllArtists() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getAllArtists();
  }

  List<RustDbStreamServer> getStreamServers() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getStreamServers();
  }

  List<RustDbStreamPlaylist> getStreamPlaylists(String serverId) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getStreamPlaylists(serverId);
  }

  void audioPlay(String source) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.audioPlay(source);
  }

  void audioPause() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.audioPause();
  }

  void audioResume() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.audioResume();
  }

  void audioStop() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.audioStop();
  }

  void audioSeek(double positionSecs) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.audioSeek(positionSecs);
  }

  void audioSetVolume(double volume) {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    bindings.audioSetVolume(volume);
  }

  RustPlaybackState getPlaybackState() {
    final bindings = _bindings ??= _RustBindings(_openLibrary());
    return bindings.getPlaybackState();
  }

  ffi.DynamicLibrary _openLibrary() {
    if (Platform.isMacOS || Platform.isIOS) {
      return ffi.DynamicLibrary.process();
    }
    if (Platform.isAndroid || Platform.isLinux) {
      return ffi.DynamicLibrary.open('librust_lib_bayin_flutter.so');
    }
    if (Platform.isWindows) {
      return ffi.DynamicLibrary.open('rust_lib_bayin_flutter.dll');
    }
    throw UnsupportedError('Unsupported platform: ${Platform.operatingSystem}');
  }
}

class _RustBindings {
  _RustBindings(ffi.DynamicLibrary library)
      : _ping = library.lookupFunction<_NativePing, _NativePing>('bayin_ping'),
        _stringFree = library.lookupFunction<
          _NativeStringFreeNative,
          _NativeStringFreeDart
        >(
          'bayin_string_free',
        ),
        _lastError = library.lookupFunction<_NativeLastError, _NativeLastError>(
          'bayin_last_error_message',
        ),
        _initDb = library.lookupFunction<_NativeBoolStringArg, _DartBoolStringArg>(
          'bayin_init_db',
        ),
        _getAllSongsJson = library.lookupFunction<_NativePing, _NativePing>(
          'bayin_db_get_all_songs_json',
        ),
        _getAllAlbumsJson = library.lookupFunction<_NativePing, _NativePing>(
          'bayin_db_get_all_albums_json',
        ),
        _getAllArtistsJson = library.lookupFunction<_NativePing, _NativePing>(
          'bayin_db_get_all_artists_json',
        ),
        _getStreamServersJson = library.lookupFunction<_NativePing, _NativePing>(
          'bayin_db_get_stream_servers_json',
        ),
        _getStreamPlaylistsJson = library.lookupFunction<
          _NativeStringStringArg,
          _DartStringStringArg
        >(
          'bayin_db_get_stream_playlists_json',
        ),
        _getMusicMetadataJson = library.lookupFunction<
          _NativeStringStringArg,
          _DartStringStringArg
        >(
          'bayin_get_music_metadata_json',
        ),
        _getLyricsJson = library.lookupFunction<_NativeStringStringArg, _DartStringStringArg>(
          'bayin_get_lyrics_json',
        ),
        _listDirectoriesJson = library.lookupFunction<
          _NativeStringStringArg,
          _DartStringStringArg
        >(
          'bayin_list_directories_json',
        ),
        _scanMusicFilesJson = library.lookupFunction<
          _NativeStringStringArg,
          _DartStringStringArg
        >(
          'bayin_scan_music_files_json',
        ),
        _scanAndSaveMusicFilesJson = library.lookupFunction<
          _NativeStringStringArg,
          _DartStringStringArg
        >(
          'bayin_scan_and_save_music_files_json',
        ),
        _clearAllSongsNoArg = library.lookupFunction<_NativeBoolNoArg, _DartBoolNoArg>(
          'bayin_db_clear_all_songs',
        ),
        _saveScanConfigJson = library.lookupFunction<_NativeBoolStringArg, _DartBoolStringArg>(
          'bayin_db_save_scan_config_json',
        ),
        _getScanConfigJson = library.lookupFunction<_NativePing, _NativePing>(
          'bayin_db_get_scan_config_json',
        ),
        _audioPlay = library.lookupFunction<_NativeBoolStringArg, _DartBoolStringArg>(
          'bayin_audio_play',
        ),
        _audioPause = library.lookupFunction<_NativeBoolNoArg, _DartBoolNoArg>(
          'bayin_audio_pause',
        ),
        _audioResume = library.lookupFunction<_NativeBoolNoArg, _DartBoolNoArg>(
          'bayin_audio_resume',
        ),
        _audioStop = library.lookupFunction<_NativeBoolNoArg, _DartBoolNoArg>(
          'bayin_audio_stop',
        ),
        _audioSeek = library.lookupFunction<_NativeBoolDoubleArg, _DartBoolDoubleArg>(
          'bayin_audio_seek',
        ),
        _audioSetVolume = library.lookupFunction<
          _NativeBoolDoubleArg,
          _DartBoolDoubleArg
        >(
          'bayin_audio_set_volume',
        ),
        _audioGetStateJson = library.lookupFunction<_NativePing, _NativePing>(
          'bayin_audio_get_state_json',
        );

  final _NativePing _ping;
  final _NativeStringFreeDart _stringFree;
  final _NativeLastError _lastError;
  final _DartBoolStringArg _initDb;
  final _NativePing _getAllSongsJson;
  final _NativePing _getAllAlbumsJson;
  final _NativePing _getAllArtistsJson;
  final _NativePing _getStreamServersJson;
  final _DartStringStringArg _getStreamPlaylistsJson;
  final _DartStringStringArg _getMusicMetadataJson;
  final _DartStringStringArg _getLyricsJson;
  final _DartStringStringArg _listDirectoriesJson;
  final _DartStringStringArg _scanMusicFilesJson;
  final _DartStringStringArg _scanAndSaveMusicFilesJson;
  final _DartBoolNoArg _clearAllSongsNoArg;
  final _DartBoolStringArg _saveScanConfigJson;
  final _NativePing _getScanConfigJson;
  final _DartBoolStringArg _audioPlay;
  final _DartBoolNoArg _audioPause;
  final _DartBoolNoArg _audioResume;
  final _DartBoolNoArg _audioStop;
  final _DartBoolDoubleArg _audioSeek;
  final _DartBoolDoubleArg _audioSetVolume;
  final _NativePing _audioGetStateJson;

  String ping() {
    final pointer = _ping();
    if (pointer == ffi.nullptr) {
      throw StateError(_takeLastError() ?? 'Rust ping() failed');
    }
    try {
      return pointer.toDartString();
    } finally {
      _stringFree(pointer);
    }
  }

  void initDb(String dbPath) {
    final pathPointer = dbPath.toNativeUtf8();
    try {
      final ok = _initDb(pathPointer);
      if (!ok) {
        throw StateError(_takeLastError() ?? 'Rust initDb() failed');
      }
    } finally {
      malloc.free(pathPointer);
    }
  }

  List<RustDirectoryEntry> listDirectories(String path) {
    final decoded = _decodeJsonList(
      _callStringArg(_listDirectoriesJson, path, 'Rust listDirectories() failed'),
    );
    return decoded
        .map(
          (item) => RustDirectoryEntry.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  List<RustScannedSong> scanMusicFiles(RustScanOptions options) {
    final decoded = _decodeJsonList(
      _callStringArg(
        _scanMusicFilesJson,
        jsonEncode(options.toJson()),
        'Rust scanMusicFiles() failed',
      ),
    );
    return decoded
        .map(
          (item) => RustScannedSong.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  RustScanAndSaveResult scanAndSaveMusicFiles(RustScanOptions options) {
    final payload = _callStringArg(
      _scanAndSaveMusicFilesJson,
      jsonEncode(options.toJson()),
      'Rust scanAndSaveMusicFiles() failed',
    );
    final decoded = jsonDecode(payload) as Map<String, dynamic>;
    return RustScanAndSaveResult.fromJson(decoded);
  }

  void clearAllSongs() {
    final ok = _clearAllSongsNoArg();
    if (!ok) {
      throw StateError(_takeLastError() ?? 'Rust clearAllSongs() failed');
    }
  }

  void saveScanConfig(RustScanConfig config) {
    final payload = jsonEncode(config.toJson()).toNativeUtf8();
    try {
      final ok = _saveScanConfigJson(payload);
      if (!ok) {
        throw StateError(_takeLastError() ?? 'Rust saveScanConfig() failed');
      }
    } finally {
      malloc.free(payload);
    }
  }

  RustScanConfig? getScanConfig() {
    final payload = _callNoArg(_getScanConfigJson, 'Rust getScanConfig() failed');
    final decoded = jsonDecode(payload);
    if (decoded == null) return null;
    return RustScanConfig.fromJson(Map<String, dynamic>.from(decoded as Map));
  }

  RustScannedSong? getMusicMetadata(String filePath) {
    final decoded = _decodeJsonValue(
      _callStringArg(_getMusicMetadataJson, filePath, 'Rust getMusicMetadata() failed'),
    );
    if (decoded == null) {
      return null;
    }
    return RustScannedSong.fromJson(decoded as Map<String, dynamic>);
  }

  String? getLyrics(String filePath) {
    final decoded = _decodeJsonValue(
      _callStringArg(_getLyricsJson, filePath, 'Rust getLyrics() failed'),
    );
    return decoded as String?;
  }

  List<RustDbSong> getAllSongs() {
    final decoded = _decodeJsonList(_callNoArg(_getAllSongsJson, 'Rust getAllSongs() failed'));
    return decoded
        .map(
          (item) => RustDbSong.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  List<RustDbAlbum> getAllAlbums() {
    final decoded = _decodeJsonList(_callNoArg(_getAllAlbumsJson, 'Rust getAllAlbums() failed'));
    return decoded
        .map(
          (item) => RustDbAlbum.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  List<RustDbArtist> getAllArtists() {
    final decoded = _decodeJsonList(_callNoArg(_getAllArtistsJson, 'Rust getAllArtists() failed'));
    return decoded
        .map(
          (item) => RustDbArtist.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  List<RustDbStreamServer> getStreamServers() {
    final decoded = _decodeJsonList(
      _callNoArg(_getStreamServersJson, 'Rust getStreamServers() failed'),
    );
    return decoded
        .map(
          (item) => RustDbStreamServer.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  List<RustDbStreamPlaylist> getStreamPlaylists(String serverId) {
    final decoded = _decodeJsonList(
      _callStringArg(
        _getStreamPlaylistsJson,
        serverId,
        'Rust getStreamPlaylists() failed',
      ),
    );
    return decoded
        .map(
          (item) => RustDbStreamPlaylist.fromJson(
            Map<String, dynamic>.from(item as Map),
          ),
        )
        .toList();
  }

  void audioPlay(String source) {
    _callBoolStringArg(_audioPlay, source, 'Rust audioPlay() failed');
  }

  void audioPause() {
    _callBoolNoArg(_audioPause, 'Rust audioPause() failed');
  }

  void audioResume() {
    _callBoolNoArg(_audioResume, 'Rust audioResume() failed');
  }

  void audioStop() {
    _callBoolNoArg(_audioStop, 'Rust audioStop() failed');
  }

  void audioSeek(double positionSecs) {
    _callBoolDoubleArg(_audioSeek, positionSecs, 'Rust audioSeek() failed');
  }

  void audioSetVolume(double volume) {
    _callBoolDoubleArg(
      _audioSetVolume,
      volume,
      'Rust audioSetVolume() failed',
    );
  }

  RustPlaybackState getPlaybackState() {
    final payload = _callNoArg(_audioGetStateJson, 'Rust getPlaybackState() failed');
    return RustPlaybackState.fromJson(
      Map<String, dynamic>.from(jsonDecode(payload) as Map),
    );
  }

  String _callNoArg(_NativePing fn, String errorMessage) {
    final pointer = fn();
    if (pointer == ffi.nullptr) {
      throw StateError(_takeLastError() ?? errorMessage);
    }
    try {
      return pointer.toDartString();
    } finally {
      _stringFree(pointer);
    }
  }

  String _callStringArg(
    _DartStringStringArg fn,
    String argument,
    String errorMessage,
  ) {
    final argPointer = argument.toNativeUtf8();
    try {
      final pointer = fn(argPointer);
      if (pointer == ffi.nullptr) {
        throw StateError(_takeLastError() ?? errorMessage);
      }
      try {
        return pointer.toDartString();
      } finally {
        _stringFree(pointer);
      }
    } finally {
      malloc.free(argPointer);
    }
  }

  void _callBoolNoArg(_DartBoolNoArg fn, String errorMessage) {
    final ok = fn();
    if (!ok) {
      throw StateError(_takeLastError() ?? errorMessage);
    }
  }

  void _callBoolStringArg(
    _DartBoolStringArg fn,
    String argument,
    String errorMessage,
  ) {
    final argPointer = argument.toNativeUtf8();
    try {
      final ok = fn(argPointer);
      if (!ok) {
        throw StateError(_takeLastError() ?? errorMessage);
      }
    } finally {
      malloc.free(argPointer);
    }
  }

  void _callBoolDoubleArg(
    _DartBoolDoubleArg fn,
    double argument,
    String errorMessage,
  ) {
    final ok = fn(argument);
    if (!ok) {
      throw StateError(_takeLastError() ?? errorMessage);
    }
  }

  List<dynamic> _decodeJsonList(String rawJson) {
    final decoded = jsonDecode(rawJson);
    if (decoded is! List<dynamic>) {
      throw StateError('Expected JSON list from Rust');
    }
    return decoded;
  }

  dynamic _decodeJsonValue(String rawJson) => jsonDecode(rawJson);

  String? _takeLastError() {
    final pointer = _lastError();
    if (pointer == ffi.nullptr) {
      return null;
    }
    try {
      final value = pointer.toDartString();
      return value.isEmpty ? null : value;
    } finally {
      _stringFree(pointer);
    }
  }
}

class RustScanOptions {
  const RustScanOptions({
    required this.directories,
    this.skipShortAudio,
    this.minDuration,
  });

  final List<String> directories;
  final bool? skipShortAudio;
  final double? minDuration;

  Map<String, dynamic> toJson() => <String, dynamic>{
        'directories': directories,
        'skipShortAudio': skipShortAudio,
        'minDuration': minDuration,
      };
}

class RustDirectoryEntry {
  const RustDirectoryEntry({
    required this.name,
    required this.path,
    required this.isDir,
  });

  factory RustDirectoryEntry.fromJson(Map<String, dynamic> json) {
    return RustDirectoryEntry(
      name: json['name'] as String,
      path: json['path'] as String,
      isDir: json['isDir'] as bool,
    );
  }

  final String name;
  final String path;
  final bool isDir;
}

class RustScannedSong {
  const RustScannedSong({
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

  factory RustScannedSong.fromJson(Map<String, dynamic> json) {
    return RustScannedSong(
      id: json['id'] as String,
      title: json['title'] as String,
      artist: json['artist'] as String,
      album: json['album'] as String,
      duration: (json['duration'] as num).toDouble(),
      filePath: json['filePath'] as String,
      fileSize: (json['fileSize'] as num).toInt(),
      coverUrl: json['coverUrl'] as String?,
      isHr: json['isHr'] as bool?,
      isSq: json['isSq'] as bool?,
      format: json['format'] as String?,
      bitDepth: (json['bitDepth'] as num?)?.toInt(),
      sampleRate: (json['sampleRate'] as num?)?.toInt(),
      bitrate: (json['bitrate'] as num?)?.toInt(),
      channels: (json['channels'] as num?)?.toInt(),
      createdAt: (json['createdAt'] as num?)?.toInt(),
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

class RustDbSong {
  const RustDbSong({
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

  factory RustDbSong.fromJson(Map<String, dynamic> json) {
    return RustDbSong(
      id: json['id'] as String,
      title: json['title'] as String,
      artist: json['artist'] as String,
      album: json['album'] as String,
      duration: (json['duration'] as num).toDouble(),
      filePath: json['filePath'] as String,
      fileSize: (json['fileSize'] as num).toInt(),
      sourceType: json['sourceType'] as String,
      createdAt: (json['createdAt'] as num).toInt(),
      updatedAt: (json['updatedAt'] as num).toInt(),
      isHr: json['isHr'] as bool?,
      isSq: json['isSq'] as bool?,
      coverHash: json['coverHash'] as String?,
      serverId: json['serverId'] as String?,
      serverSongId: json['serverSongId'] as String?,
      streamInfo: json['streamInfo'] as String?,
      fileModified: (json['fileModified'] as num?)?.toInt(),
      format: json['format'] as String?,
      bitDepth: (json['bitDepth'] as num?)?.toInt(),
      sampleRate: (json['sampleRate'] as num?)?.toInt(),
      bitrate: (json['bitrate'] as num?)?.toInt(),
      channels: (json['channels'] as num?)?.toInt(),
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

class RustDbAlbum {
  const RustDbAlbum({
    required this.id,
    required this.name,
    required this.artist,
    required this.songCount,
    this.coverHash,
    this.streamCoverUrl,
  });

  factory RustDbAlbum.fromJson(Map<String, dynamic> json) {
    return RustDbAlbum(
      id: json['id'] as String,
      name: json['name'] as String,
      artist: json['artist'] as String,
      songCount: (json['songCount'] as num).toInt(),
      coverHash: json['coverHash'] as String?,
      streamCoverUrl: json['streamCoverUrl'] as String?,
    );
  }

  final String id;
  final String name;
  final String artist;
  final String? coverHash;
  final String? streamCoverUrl;
  final int songCount;
}

class RustDbArtist {
  const RustDbArtist({
    required this.id,
    required this.name,
    required this.songCount,
    this.coverHash,
    this.streamCoverUrl,
  });

  factory RustDbArtist.fromJson(Map<String, dynamic> json) {
    return RustDbArtist(
      id: json['id'] as String,
      name: json['name'] as String,
      songCount: (json['songCount'] as num).toInt(),
      coverHash: json['coverHash'] as String?,
      streamCoverUrl: json['streamCoverUrl'] as String?,
    );
  }

  final String id;
  final String name;
  final String? coverHash;
  final String? streamCoverUrl;
  final int songCount;
}

class RustDbStreamServer {
  const RustDbStreamServer({
    required this.id,
    required this.serverType,
    required this.serverName,
    required this.serverUrl,
    required this.username,
    required this.password,
    required this.enabled,
    required this.createdAt,
    this.accessToken,
    this.userId,
  });

  factory RustDbStreamServer.fromJson(Map<String, dynamic> json) {
    return RustDbStreamServer(
      id: json['id'] as String,
      serverType: json['serverType'] as String,
      serverName: json['serverName'] as String,
      serverUrl: json['serverUrl'] as String,
      username: json['username'] as String,
      password: json['password'] as String,
      accessToken: json['accessToken'] as String?,
      userId: json['userId'] as String?,
      enabled: json['enabled'] as bool,
      createdAt: (json['createdAt'] as num).toInt(),
    );
  }

  final String id;
  final String serverType;
  final String serverName;
  final String serverUrl;
  final String username;
  final String password;
  final String? accessToken;
  final String? userId;
  final bool enabled;
  final int createdAt;
}

class RustDbStreamPlaylist {
  const RustDbStreamPlaylist({
    required this.serverId,
    required this.playlistId,
    required this.name,
    required this.songCount,
    required this.kind,
    required this.syncedAt,
    this.updatedAt,
  });

  factory RustDbStreamPlaylist.fromJson(Map<String, dynamic> json) {
    return RustDbStreamPlaylist(
      serverId: json['serverId'] as String,
      playlistId: json['playlistId'] as String,
      name: json['name'] as String,
      songCount: (json['songCount'] as num).toInt(),
      kind: json['kind'] as String,
      updatedAt: (json['updatedAt'] as num?)?.toInt(),
      syncedAt: (json['syncedAt'] as num).toInt(),
    );
  }

  final String serverId;
  final String playlistId;
  final String name;
  final int songCount;
  final String kind;
  final int? updatedAt;
  final int syncedAt;
}

class RustScanAndSaveResult {
  const RustScanAndSaveResult({
    required this.scanned,
    required this.saved,
    required this.skipped,
  });

  factory RustScanAndSaveResult.fromJson(Map<String, dynamic> json) {
    return RustScanAndSaveResult(
      scanned: (json['scanned'] as num).toInt(),
      saved: (json['saved'] as num).toInt(),
      skipped: (json['skipped'] as num).toInt(),
    );
  }

  final int scanned;
  final int saved;
  final int skipped;
}

class RustScanConfig {
  const RustScanConfig({
    required this.directories,
    required this.skipShort,
    required this.minDuration,
    this.id,
    this.lastScanAt,
  });

  factory RustScanConfig.fromJson(Map<String, dynamic> json) {
    final dirs = (json['directories'] as List<dynamic>? ?? const <dynamic>[])
        .map((d) => d as String)
        .toList();
    return RustScanConfig(
      id: (json['id'] as num?)?.toInt(),
      directories: dirs,
      skipShort: json['skipShort'] as bool? ?? false,
      minDuration: (json['minDuration'] as num?)?.toDouble() ?? 30.0,
      lastScanAt: (json['lastScanAt'] as num?)?.toInt(),
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'directories': directories,
        'skipShort': skipShort,
        'minDuration': minDuration,
        'lastScanAt': lastScanAt,
      };

  final int? id;
  final List<String> directories;
  final bool skipShort;
  final double minDuration;
  final int? lastScanAt;
}

class RustPlaybackState {
  const RustPlaybackState({
    required this.isPlaying,
    required this.positionSecs,
    required this.durationSecs,
    required this.volume,
    required this.currentSource,
    required this.hasEnded,
  });

  factory RustPlaybackState.fromJson(Map<String, dynamic> json) {
    return RustPlaybackState(
      isPlaying: json['isPlaying'] as bool? ?? false,
      positionSecs: (json['positionSecs'] as num?)?.toDouble() ?? 0,
      durationSecs: (json['durationSecs'] as num?)?.toDouble() ?? 0,
      volume: (json['volume'] as num?)?.toDouble() ?? 1,
      currentSource: json['currentSource'] as String?,
      hasEnded: json['hasEnded'] as bool? ?? false,
    );
  }

  final bool isPlaying;
  final double positionSecs;
  final double durationSecs;
  final double volume;
  final String? currentSource;
  final bool hasEnded;
}
