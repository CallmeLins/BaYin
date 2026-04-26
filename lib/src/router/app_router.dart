import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../pages/about/about_page.dart';
import '../pages/about/creators_page.dart';
import '../pages/about/donate_page.dart';
import '../pages/about/licenses_page.dart';
import '../pages/about/official_website_page.dart';
import '../pages/about/privacy_page.dart';
import '../pages/about/terms_page.dart';
import '../pages/albums/album_detail_page.dart';
import '../pages/albums/albums_page.dart';
import '../pages/artists/artist_detail_page.dart';
import '../pages/artists/artists_page.dart';
import '../pages/debug/debug_page.dart';
import '../pages/library/music_library_page.dart';
import '../pages/player/player_page.dart';
import '../pages/playlists/playlist_detail_page.dart';
import '../pages/playlists/playlists_page.dart';
import '../pages/scan/scan_music_page.dart';
import '../pages/search/search_page.dart';
import '../pages/settings/bayin_pro_page.dart';
import '../pages/settings/equalizer_settings_page.dart';
import '../pages/settings/help_feedback_page.dart';
import '../pages/settings/lyric_settings_page.dart';
import '../pages/settings/settings_page.dart';
import '../pages/settings/update_software_page.dart';
import '../pages/settings/user_interface_page.dart';
import '../pages/songs/songs_page.dart';
import '../pages/stream/stream_playlist_detail_page.dart';
import '../pages/stream/stream_server_config_page.dart';
import '../widgets/widgets.dart';

final appRouterProvider = Provider<GoRouter>((ref) {
  return GoRouter(
    initialLocation: '/',
    routes: <RouteBase>[
      ShellRoute(
        builder: (context, state, child) => RootScaffold(child: child),
        routes: <RouteBase>[
          GoRoute(path: '/', builder: (_, _) => const SongsPage()),
          GoRoute(path: '/search', builder: (_, _) => const SearchPage()),
          GoRoute(path: '/scan', builder: (_, _) => const ScanMusicPage()),
          GoRoute(path: '/player', builder: (_, _) => const PlayerPage()),
          GoRoute(path: '/albums', builder: (_, _) => const AlbumsPage()),
          GoRoute(
            path: '/albums/:albumId',
            builder: (_, _) => const AlbumDetailPage(),
          ),
          GoRoute(path: '/artists', builder: (_, _) => const ArtistsPage()),
          GoRoute(
            path: '/artists/:artistId',
            builder: (_, _) => const ArtistDetailPage(),
          ),
          GoRoute(path: '/playlists', builder: (_, _) => const PlaylistsPage()),
          GoRoute(
            path: '/playlists/:playlistId',
            builder: (_, _) => const PlaylistDetailPage(),
          ),
          GoRoute(
            path: '/stream-playlists/:serverId/:playlistId',
            builder: (_, _) => const StreamPlaylistDetailPage(),
          ),
          GoRoute(
            path: '/library',
            builder: (_, _) => const MusicLibraryPage(),
          ),
          GoRoute(
            path: '/stream-config',
            builder: (_, _) => const StreamServerConfigPage(),
          ),
          GoRoute(path: '/settings', builder: (_, _) => const SettingsPage()),
          GoRoute(
            path: '/settings/pro',
            builder: (_, _) => const BayinProPage(),
          ),
          GoRoute(
            path: '/settings/interface',
            builder: (_, _) => const UserInterfacePage(),
          ),
          GoRoute(
            path: '/settings/lyrics',
            builder: (_, _) => const LyricSettingsPage(),
          ),
          GoRoute(
            path: '/settings/equalizer',
            builder: (_, _) => const EqualizerSettingsPage(),
          ),
          GoRoute(
            path: '/settings/help',
            builder: (_, _) => const HelpFeedbackPage(),
          ),
          GoRoute(
            path: '/settings/update',
            builder: (_, _) => const UpdateSoftwarePage(),
          ),
          GoRoute(path: '/about', builder: (_, _) => const AboutPage()),
          GoRoute(
            path: '/about/creators',
            builder: (_, _) => const CreatorsPage(),
          ),
          GoRoute(
            path: '/about/terms',
            builder: (_, _) => const TermsPage(),
          ),
          GoRoute(
            path: '/about/privacy',
            builder: (_, _) => const PrivacyPage(),
          ),
          GoRoute(
            path: '/about/licenses',
            builder: (_, _) => const LicensesPage(),
          ),
          GoRoute(
            path: '/about/donate',
            builder: (_, _) => const DonatePage(),
          ),
          GoRoute(
            path: '/about/website',
            builder: (_, _) => const OfficialWebsitePage(),
          ),
          GoRoute(path: '/debug', builder: (_, _) => const DebugPage()),
        ],
      ),
    ],
  );
});
