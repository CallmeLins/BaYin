import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../services/library_service.dart';
import '../../widgets/widgets.dart';

class StreamPlaylistDetailPage extends ConsumerStatefulWidget {
  const StreamPlaylistDetailPage({super.key});

  @override
  ConsumerState<StreamPlaylistDetailPage> createState() =>
      _StreamPlaylistDetailPageState();
}

class _StreamPlaylistDetailPageState
    extends ConsumerState<StreamPlaylistDetailPage> {
  String? _serverId;
  String? _playlistId;

  bool _isLoading = false;
  bool _isSyncing = false;
  String? _error;
  List<Song> _songs = const <Song>[];

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final state = GoRouterState.of(context);
    final rawServerId = state.pathParameters['serverId'];
    final rawPlaylistId = state.pathParameters['playlistId'];
    if (rawServerId == null || rawPlaylistId == null) {
      return;
    }

    final serverId = Uri.decodeComponent(rawServerId);
    final playlistId = Uri.decodeComponent(rawPlaylistId);
    if (_serverId == serverId && _playlistId == playlistId) {
      return;
    }
    _serverId = serverId;
    _playlistId = playlistId;
    _loadSongs();
  }

  @override
  Widget build(BuildContext context) {
    final serverId = _serverId;
    final playlistId = _playlistId;
    if (serverId == null || playlistId == null) {
      return const Center(child: Text('Invalid stream playlist route.'));
    }

    final asyncServers = ref.watch(streamServersProvider);
    final asyncPlaylists = ref.watch(streamPlaylistsProvider(serverId));

    final serverName = asyncServers.maybeWhen(
      data: (servers) {
        for (final server in servers) {
          if (server.id == serverId) return server.serverName;
        }
        return serverId;
      },
      orElse: () => serverId,
    );

    final playlistName = asyncPlaylists.maybeWhen(
      data: (playlists) {
        for (final playlist in playlists) {
          if (playlist.id == playlistId) return playlist.name;
        }
        return playlistId;
      },
      orElse: () => playlistId,
    );

    return Column(
      children: [
        BayinPageHeader(
          title: Text(playlistName),
          left: IconButton(
            onPressed: () => context.go('/playlists'),
            icon: Icon(PhosphorIcons.caretLeft()),
            tooltip: 'Back to playlists',
          ),
          right: IconButton(
            onPressed: _isSyncing ? null : _handleSyncAndReload,
            icon: _isSyncing
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : Icon(PhosphorIcons.arrowsClockwise()),
            tooltip: 'Sync and refresh',
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(14, 0, 14, 10),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(
              serverName,
              style: TextStyle(
                fontSize: 12,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ),
        Expanded(
          child: BayinGlassCard(
            margin: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            child: _buildBody(context),
          ),
        ),
      ],
    );
  }

  Widget _buildBody(BuildContext context) {
    if (_isLoading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: SelectableText(
            _error!,
            textAlign: TextAlign.center,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        ),
      );
    }
    if (_songs.isEmpty) {
      return Center(
        child: Text(
          'No songs in this stream playlist.',
          style: TextStyle(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
      );
    }
    return SongList(
      songs: _songs,
      showIndex: true,
      onTap: (song) {
        final index = _songs.indexOf(song);
        if (index < 0) return;
        ref
            .read(playerControllerProvider.notifier)
            .playQueue(_songs, startIndex: index);
      },
      onLongPress: (song) => SongMenu.show(context, song: song),
    );
  }

  Future<void> _loadSongs() async {
    final serverId = _serverId;
    final playlistId = _playlistId;
    if (serverId == null || playlistId == null) return;
    setState(() {
      _isLoading = true;
      _error = null;
    });
    try {
      final songs = await LibraryService.instance.loadStreamPlaylistSongs(
        serverId,
        playlistId,
      );
      if (!mounted) return;
      setState(() => _songs = songs);
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _songs = const <Song>[];
        _error = 'Failed to load stream playlist songs:\n$error';
      });
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  Future<void> _handleSyncAndReload() async {
    final serverId = _serverId;
    if (serverId == null || _isSyncing) return;
    setState(() => _isSyncing = true);
    try {
      await LibraryService.instance.syncStreamPlaylists(serverId);
      if (!mounted) return;
      ref.invalidate(streamPlaylistsProvider(serverId));
      await _loadSongs();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Playlists synced.')),
      );
    } catch (error) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Sync failed: $error')),
      );
    } finally {
      if (mounted) {
        setState(() => _isSyncing = false);
      }
    }
  }
}
