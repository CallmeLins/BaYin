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
              right: BayinGhostIconButton(
                icon: PhosphorIcons.cloud(),
                tooltip: 'Configure',
                onTap: () => context.go('/stream-config'),
              ),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
                children: [
                  Container(
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: Theme.of(context).brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.03)
                          : Colors.black.withValues(alpha: 0.02),
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(
                        color: Theme.of(context).brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.07)
                            : Colors.black.withValues(alpha: 0.06),
                        width: 0.6,
                      ),
                    ),
                    child: Column(
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
                      ],
                    ),
                  ),
                  const SizedBox(height: 16),
                  Container(
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: Theme.of(context).brightness == Brightness.dark
                          ? Colors.white.withValues(alpha: 0.03)
                          : Colors.black.withValues(alpha: 0.02),
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(
                        color: Theme.of(context).brightness == Brightness.dark
                            ? Colors.white.withValues(alpha: 0.07)
                            : Colors.black.withValues(alpha: 0.06),
                        width: 0.6,
                      ),
                    ),
                    child: Column(
                      children: [
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
                              padding: const EdgeInsets.only(bottom: 12),
                              child: _StreamServerBlock(server: server),
                            ),
                      ],
                    ),
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
      padding: const EdgeInsets.only(bottom: 10),
      child: Row(
        children: [
          Text(title, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w600)),
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
        borderRadius: BorderRadius.circular(12),
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
            size: 18,
            color: tokens.textSecondary,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              'No local playlists yet. User-created playlists land with the Phase 3 follow-up.',
              style: TextStyle(
                color: tokens.textSecondary,
                fontSize: 12,
                height: 1.35,
                fontWeight: FontWeight.w500,
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
        borderRadius: BorderRadius.circular(12),
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
            size: 18,
            color: tokens.textSecondary,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              'No stream servers configured. Add a Subsonic/Jellyfin server in Phase 6.',
              style: TextStyle(
                color: tokens.textSecondary,
                fontSize: 12,
                height: 1.35,
                fontWeight: FontWeight.w500,
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
    final brightness = Theme.of(context).brightness;
    return Container(
      padding: const EdgeInsets.fromLTRB(10, 10, 10, 8),
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.025)
            : Colors.black.withValues(alpha: 0.015),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                PhosphorIcons.cloud(),
                size: 16,
                color: tokens.textSecondary,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  server.serverName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
                ),
              ),
              const SizedBox(width: 6),
              Text(
                '(${server.serverType})',
                style: TextStyle(
                  color: tokens.textSecondary,
                  fontSize: 11,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Divider(
            height: 1,
            thickness: 1,
            color: brightness == Brightness.dark
                ? Colors.white.withValues(alpha: 0.08)
                : Colors.black.withValues(alpha: 0.06),
          ),
          const SizedBox(height: 8),
          asyncPlaylists.when(
            loading: () => const LinearProgressIndicator(),
            error: (error, _) => Text(
              'Failed to load playlists: $error',
              style: const TextStyle(color: Colors.red),
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
      ),
    );
  }
}

class _StreamPlaylistRow extends ConsumerStatefulWidget {
  const _StreamPlaylistRow({
    required this.playlist,
    required this.serverId,
  });

  final Playlist playlist;
  final String serverId;

  @override
  ConsumerState<_StreamPlaylistRow> createState() => _StreamPlaylistRowState();
}

class _StreamPlaylistRowState extends ConsumerState<_StreamPlaylistRow> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final tokens = ref.watch(bayinTokensProvider);
    final brightness = Theme.of(context).brightness;
    final hoverBg = brightness == Brightness.dark
        ? Colors.white.withValues(alpha: 0.06)
        : Colors.black.withValues(alpha: 0.04);

    return MouseRegion(
      onEnter: (_) => setState(() => _hovering = true),
      onExit: (_) => setState(() => _hovering = false),
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        onTap: () => context.go(
          '/stream-playlists/${Uri.encodeComponent(widget.serverId)}'
          '/${Uri.encodeComponent(widget.playlist.id)}',
        ),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          curve: Curves.easeOut,
          decoration: BoxDecoration(
            color: _hovering ? hoverBg : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
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
                    widget.playlist.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
                  ),
                ),
                Text(
                  '${widget.playlist.songCount} tracks',
                  style: TextStyle(
                    fontSize: 11,
                    color: tokens.textSecondary,
                    fontFeatures: const [FontFeature.tabularFigures()],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
