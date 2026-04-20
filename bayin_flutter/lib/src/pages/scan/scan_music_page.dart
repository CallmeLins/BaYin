import 'package:flutter/widgets.dart';

import '../../widgets/widgets.dart';

/// Stub — real implementation lands in Phase 7 (full scan settings + folder
/// picker). The FFI smoke-test flow lives at `/debug` in the meantime.
class ScanMusicPage extends StatelessWidget {
  const ScanMusicPage({super.key});

  @override
  Widget build(BuildContext context) {
    return const PlaceholderPage(
      title: 'Scan music',
      phase: 'Phase 7',
      note: 'FFI scan smoke test lives at /debug for now.',
    );
  }
}
