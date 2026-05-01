import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../models/models.dart';
import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

/// Phase 3 — playlists overview (local placeholder + stream-server cached ones).
///
/// Local playlist CRUD is Phase 3+ (not implemented yet on the Rust side);
/// stream playlists come from `streamPlaylistsProvider(serverId)`.
class PlaylistsPage extends ConsumerWidget {
  const PlaylistsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncServers = ref.watch(streamServersProvider);
    return asyncServers.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, _) => Center(
        child: SelectableText('Failed to load stream servers\n$error'),
      ),
      data: (servers) {
        return Column(
          children: [
            BayinPageHeader(
              title: const Text('Playlists'),
              right: Tooltip(
                message: 'Configure',
                child: IconButton(
                  icon: Icon(PhosphorIcons.cloud()),
                  onPressed: () => context.go('/stream-config'),
                ),
              ),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
                children: [
                  _SectionHeader(
                    title: 'Local playlists',
                    trailing: FilledButton(
                      onPressed: null,
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(PhosphorIcons.plus()),
                          const SizedBox(width: 8),
                          const Text('New'),
                        ],
                      ),
                    ),
                  ),
                  const _LocalPlaylistsPlaceholder(),
                  const SizedBox(height: 24),
                  _SectionHeader(
                    title: 'Stream servers',
                    trailing: FilledButton(
                      onPressed: () => context.go('/stream-config'),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(PhosphorIcons.cloud()),
                          const SizedBox(width: 8),
                          const Text('Configure'),
                        ],
                      ),
                    ),
                  ),
                  if (servers.isEmpty)
                    const _NoStreamServersPlaceholder()
                  else
                    for (final server in servers)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 16),
                        child: _StreamServerBlock(server: server),
                      ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }
}

class _SectionHeader extends ConsumerWidget {
  const _SectionHeader({required this.title, this.trailing});

  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        children: [
          Text(title, style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w600)),
          const Spacer(),
          ...[trailing].nonNulls,
        ],
      ),
    );
  }
}

class _LocalPlaylistsPlaceholder extends ConsumerWidget {
  const _LocalPlaylistsPlaceholder();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final isDark = tokens.isDark;
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: isDark
            ? Colors.white.withValues(alpha: 0.05)
            : Colors.black.withValues(alpha: 0.03),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: isDark
              ? Colors.white.withValues(alpha: 0.07)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.6,
        ),
      ),
      child: Row(
        children: [
          Icon(
            PhosphorIcons.playlist(),
            color: tokens.textSecondary,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              'No local playlists yet. User-created playlists land with the Phase 3 follow-up.',
              style: TextStyle(
                color: tokens.textSecondary,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _NoStreamServersPlaceholder extends ConsumerWidget {
  const _NoStreamServersPlaceholder();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final isDark = tokens.isDark;
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: isDark
            ? Colors.white.withValues(alpha: 0.05)
            : Colors.black.withValues(alpha: 0.03),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: isDark
              ? Colors.white.withValues(alpha: 0.07)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.6,
        ),
      ),
      child: Row(
        children: [
          Icon(
            PhosphorIcons.cloudSlash(),
            color: tokens.textSecondary,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              'No stream servers configured. Add a Subsonic/Jellyfin server in Phase 6.',
              style: TextStyle(
                color: tokens.textSecondary,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _StreamServerBlock extends ConsumerWidget {
  const _StreamServerBlock({required this.server});

  final StreamServer server;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    final asyncPlaylists = ref.watch(streamPlaylistsProvider(server.id));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(PhosphorIcons.cloud(), size: 16),
            const SizedBox(width: 6),
            Text(
              server.serverName,
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
            const SizedBox(width: 6),
            Text(
              '(${server.serverType})',
              style: TextStyle(
                color: tokens.textSecondary,
                fontSize: 12,
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        asyncPlaylists.when(
          loading: () => const LinearProgressIndicator(),
          error: (error, _) => Text(
            'Failed to load playlists: $error',
            style: TextStyle(color: Colors.red),
          ),
          data: (playlists) {
            if (playlists.isEmpty) {
              return Padding(
                padding: const EdgeInsets.only(left: 22),
                child: Text(
                  'No cached playlists for this server.',
                  style: TextStyle(
                    color: tokens.textSecondary,
                  ),
                ),
              );
            }
            return Column(
              children: [
                for (final playlist in playlists)
                  _StreamPlaylistRow(
                    playlist: playlist,
                    serverId: server.id,
                  ),
              ],
            );
          },
        ),
      ],
    );
  }
}

class _StreamPlaylistRow extends ConsumerWidget {
  const _StreamPlaylistRow({
    required this.playlist,
    required this.serverId,
  });

  final Playlist playlist;
  final String serverId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = ref.watch(bayinTokensProvider);
    return GestureDetector(
      onTap: () => context.go(
        '/stream-playlists/${Uri.encodeComponent(serverId)}'
        '/${Uri.encodeComponent(playlist.id)}',
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(22, 6, 10, 6),
        child: Row(
          children: [
            Icon(
              PhosphorIcons.playlist(),
              size: 18,
              color: tokens.textSecondary,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                playlist.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 13),
              ),
            ),
            Text(
              '${playlist.songCount} tracks',
              style: TextStyle(
                fontSize: 11,
                color: tokens.textSecondary,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
