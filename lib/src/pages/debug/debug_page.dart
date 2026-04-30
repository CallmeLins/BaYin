import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/providers.dart';
import '../../rust/rust_api.dart';
import '../../widgets/widgets.dart';

/// FFI smoke-test page (/debug).
///
/// Kept from Phase 0 bring-up as a developer surface: it exercises the Rust
/// `ping()` FFI, DB init, library counters, and a one-shot directory scan.
/// Replaced by real flows in later phases — this page is just to verify the
/// pure-FFI bridge stays healthy while other features land.
class DebugPage extends ConsumerStatefulWidget {
  const DebugPage({super.key});

  @override
  ConsumerState<DebugPage> createState() => _DebugPageState();
}

class _DebugPageState extends ConsumerState<DebugPage> {
  late final TextEditingController _directoryController;

  @override
  void initState() {
    super.initState();
    _directoryController = TextEditingController();
  }

  @override
  void dispose() {
    _directoryController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final tokens = ref.watch(bayinTokensProvider);
    final pong = RustApi.instance.ping();
    final dbPath = ref.watch(databasePathProvider);
    final songs = ref.watch(librarySongsProvider);
    final albums = ref.watch(libraryAlbumsProvider);
    final artists = ref.watch(libraryArtistsProvider);
    final scanner = ref.watch(scannerProvider);

    return ListView(
      padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
      children: [
        const BayinPageHeader(title: Text('Debug')),
        BayinGlassCard(
          margin: const EdgeInsets.fromLTRB(0, 6, 0, 0),
          padding: const EdgeInsets.all(16),
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 720),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'BaYin Flutter edition',
                    textAlign: TextAlign.center,
                    style: TextStyle(fontSize: 20, fontWeight: FontWeight.w600),
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'Rust FFI roundtrip: $pong',
                    textAlign: TextAlign.center,
                    style: TextStyle(fontSize: 14, color: tokens.textSecondary),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    dbPath.when(
                      data: (path) => 'Database: $path',
                      loading: () => 'Database: initializing...',
                      error: (error, _) => 'Database init failed: $error',
                    ),
                    textAlign: TextAlign.center,
                    style: TextStyle(fontSize: 12, color: tokens.textSecondary),
                  ),
                  const SizedBox(height: 24),
              _StatLine(
                label: 'Library songs',
                value: songs.when(
                  data: (items) => '${items.length}',
                  loading: () => 'loading...',
                  error: (error, _) => 'error: $error',
                ),
              ),
              _StatLine(
                label: 'Albums',
                value: albums.when(
                  data: (items) => '${items.length}',
                  loading: () => 'loading...',
                  error: (error, _) => 'error: $error',
                ),
              ),
              _StatLine(
                label: 'Artists',
                value: artists.when(
                  data: (items) => '${items.length}',
                  loading: () => 'loading...',
                  error: (error, _) => 'error: $error',
                ),
              ),
              const SizedBox(height: 24),
              const Text(
                'Directory scan smoke test',
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 8),
              TextBox(
                controller: _directoryController,
                placeholder: r'Enter a local music directory, e.g. D:\Music',
              ),
              const SizedBox(height: 8),
              FilledButton(
                onPressed: scanner.isLoading
                    ? null
                    : () async {
                        final path = _directoryController.text.trim();
                        if (path.isEmpty) {
                          return;
                        }
                        await ref
                            .read(scannerProvider.notifier)
                            .scanDirectories(<String>[path]);
                      },
                child: Text(scanner.isLoading ? 'Scanning...' : 'Scan directory'),
              ),
              const SizedBox(height: 8),
              OutlinedButton(
                onPressed: scanner.isLoading
                    ? null
                    : () async {
                        final path = _directoryController.text.trim();
                        if (path.isEmpty) {
                          return;
                        }
                        await ref
                            .read(scannerProvider.notifier)
                            .scanAndSave(<String>[path]);
                      },
                child: Text(scanner.isLoading ? 'Working...' : 'Scan & save to DB'),
              ),
              const SizedBox(height: 8),
              HyperlinkButton(
                onPressed: scanner.isLoading
                    ? null
                    : () => ref.read(scannerProvider.notifier).clearLibrary(),
                child: const Text('Clear all local songs'),
              ),
              const SizedBox(height: 8),
              _StatLine(
                label: 'Scan results',
                value: '${scanner.results.length}',
              ),
              if (scanner.lastSave != null)
                _StatLine(
                  label: 'Last save',
                  value:
                      'scanned ${scanner.lastSave!.scanned} / saved ${scanner.lastSave!.saved}',
                ),
              if (scanner.directories.isNotEmpty)
                _StatLine(
                  label: 'Last scan path',
                  value: scanner.directories.join(', '),
                ),
              if (scanner.results.isNotEmpty)
                _StatLine(
                  label: 'First result',
                  value:
                      '${scanner.results.first.title} - ${scanner.results.first.artist}',
                ),
              if (scanner.error != null)
                Text(
                  'Scan failed: ${scanner.error}',
                  style: const TextStyle(color: Color(0xFFE81123)),
                ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _StatLine extends StatelessWidget {
  const _StatLine({
    required this.label,
    required this.value,
  });

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Text('$label: $value'),
    );
  }
}
