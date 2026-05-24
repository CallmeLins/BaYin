import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/models.dart';
import '../services/library_service.dart';

final libraryServiceProvider = Provider<LibraryService>((ref) {
  return LibraryService.instance;
});

final databasePathProvider = FutureProvider<String>((ref) async {
  final service = ref.watch(libraryServiceProvider);
  return service.ensureDatabaseInitialized();
});

final librarySongsProvider = FutureProvider<List<Song>>((ref) async {
  await ref.watch(databasePathProvider.future);
  final service = ref.watch(libraryServiceProvider);
  return service.loadSongs();
});

final libraryAlbumsProvider = FutureProvider<List<Album>>((ref) async {
  await ref.watch(databasePathProvider.future);
  final service = ref.watch(libraryServiceProvider);
  return service.loadAlbums();
});

final libraryArtistsProvider = FutureProvider<List<Artist>>((ref) async {
  await ref.watch(databasePathProvider.future);
  final service = ref.watch(libraryServiceProvider);
  return service.loadArtists();
});

final streamServersProvider = FutureProvider<List<StreamServer>>((ref) async {
  await ref.watch(databasePathProvider.future);
  final service = ref.watch(libraryServiceProvider);
  return service.loadStreamServers();
});

final streamPlaylistsProvider =
    FutureProvider.family<List<Playlist>, String>((ref, serverId) async {
  await ref.watch(databasePathProvider.future);
  final service = ref.watch(libraryServiceProvider);
  return service.loadStreamPlaylists(serverId);
});
