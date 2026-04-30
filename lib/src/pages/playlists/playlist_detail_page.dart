import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../widgets/widgets.dart';

class PlaylistDetailPage extends StatelessWidget {
  const PlaylistDetailPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Playlist detail'),
          left: IconButton(
            tooltip: 'Back',
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/playlists');
              }
            },
            icon: Icon(PhosphorIcons.caretLeft()),
          ),
        ),
        Expanded(
          child: Center(
            child: BayinGlassCard(
              margin: const EdgeInsets.all(12),
              padding: const EdgeInsets.all(18),
              child: Text(
                'Local playlist detail is not wired yet in Flutter. This route is reserved for parity with Tauri playlist detail.',
                style: TextStyle(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
