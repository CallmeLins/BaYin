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
              right: IconButton(
                icon: Icon(PhosphorIcons.cloud()),
                tooltip: 'Configure',
                onPressed: () => context.go('/stream-config'),
              ),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
                children: [
                  _SectionHeader(
                    title: 'Local playlists',
                    trailing: TextButton.icon(
                      icon: Icon(PhosphorIcons.plus()),
                      label: const Text('New'),
                      onPressed: null,
                    ),
                  ),
                  const _LocalPlaylistsPlaceholder(),
                  const SizedBox(height: 24),
                  _SectionHeader(
                    title: 'Stream servers',
                    trailing: TextButton.icon(
                      icon: Icon(PhosphorIcons.cloud()),
                      label: const Text('Configure'),
                      onPressed: () => context.go('/stream-config'),
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

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({required this.title, this.trailing});

  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        children: [
          Text(title, style: Theme.of(context).textTheme.titleLarge),
          const Spacer(),
          ...[trailing].nonNulls,
        ],
      ),
    );
  }
}

class _LocalPlaylistsPlaceholder extends StatelessWidget {
  const _LocalPlaylistsPlaceholder();

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: dark
            ? Colors.white.withValues(alpha: 0.05)
            : Colors.black.withValues(alpha: 0.03),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: dark
              ? Colors.white.withValues(alpha: 0.07)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.6,
        ),
      ),
      child: Row(
        children: [
          Icon(
            PhosphorIcons.playlist(),
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              'No local playlists yet. User-created playlists land with the Phase 3 follow-up.',
              style: TextStyle(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _NoStreamServersPlaceholder extends StatelessWidget {
  const _NoStreamServersPlaceholder();

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: dark
            ? Colors.white.withValues(alpha: 0.05)
            : Colors.black.withValues(alpha: 0.03),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: dark
              ? Colors.white.withValues(alpha: 0.07)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.6,
        ),
      ),
      child: Row(
        children: [
          Icon(
            PhosphorIcons.cloudSlash(),
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              'No stream servers configured. Add a Subsonic/Jellyfin server in Phase 6.',
              style: TextStyle(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
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
                color: Theme.of(context).colorScheme.onSurfaceVariant,
                fontSize: 12,
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        asyncPlaylists.when(
          loading: () => const LinearProgressIndicator(minHeight: 2),
          error: (error, _) => Text(
            'Failed to load playlists: $error',
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
          data: (playlists) {
            if (playlists.isEmpty) {
              return Padding(
                padding: const EdgeInsets.only(left: 22),
                child: Text(
                  'No cached playlists for this server.',
                  style: TextStyle(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
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

class _StreamPlaylistRow extends StatelessWidget {
  const _StreamPlaylistRow({
    required this.playlist,
    required this.serverId,
  });

  final Playlist playlist;
  final String serverId;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
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
                color: Theme.of(context).colorScheme.onSurfaceVariant,
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
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
