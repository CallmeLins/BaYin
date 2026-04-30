import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:phosphor_flutter/phosphor_flutter.dart';

import '../../providers/providers.dart';
import '../../widgets/widgets.dart';

class EqualizerSettingsPage extends ConsumerWidget {
  const EqualizerSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final app = ref.watch(appSettingsProvider);
    final controller = ref.read(appSettingsProvider.notifier);
    final tokens = ref.watch(bayinTokensProvider);
    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Equalizer'),
          left: IconButton(
            onPressed: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
            icon: Icon(PhosphorIcons.caretLeft()),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 10),
            children: [
              const SizedBox(height: 10),
              const SizedBox(height: 6),
              Text(
                '10-band gain profile with Rust engine hook enabled.',
                style: TextStyle(color: tokens.textSecondary),
              ),
              const SizedBox(height: 12),
              EqualizerPanel(
                enabled: app.eqEnabled,
                selectedPreset: app.eqPreset,
                gains: app.eqGains,
                onEnabledChanged: controller.setEqEnabled,
                onBandGainChanged: controller.setEqBandGain,
                onPresetChanged: (preset) => _applyPreset(controller, preset),
                onReset: controller.resetEq,
              ),
            ],
          ),
        ),
      ],
    );
  }

  void _applyPreset(AppSettingsController controller, String preset) {
    controller.setEqPreset(preset);
    if (preset == 'balance') {
      controller.resetEq();
      return;
    }

    if (preset == 'bass') {
      _setAll(controller, const [6, 5, 4, 2, 1, 0, -1, -2, -3, -4]);
      return;
    }
    if (preset == 'vocal') {
      _setAll(controller, const [-2, -1, 0, 2, 4, 5, 4, 2, 0, -1]);
      return;
    }
    if (preset == 'bright') {
      _setAll(controller, const [-3, -2, -1, 0, 1, 2, 4, 5, 6, 6]);
    }
  }

  void _setAll(AppSettingsController controller, List<double> gains) {
    for (var i = 0; i < gains.length; i++) {
      controller.setEqBandGain(i, gains[i]);
    }
  }
}
