import 'package:flutter/material.dart';
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

    return Column(
      children: [
        BayinPageHeader(
          title: const Text('Equalizer'),
          left: BayinGhostIconButton(
            icon: PhosphorIcons.caretLeft(),
            tooltip: 'Back',
            onTap: () {
              if (context.canPop()) {
                context.pop();
              } else {
                context.go('/settings');
              }
            },
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              const SizedBox(height: 8),
              const BayinSectionHeader(title: 'EQUALIZER'),
              _GlassWrap(
                child: Padding(
                  padding: const EdgeInsets.all(14),
                  child: EqualizerPanel(
                    enabled: app.eqEnabled,
                    selectedPreset: app.eqPreset,
                    gains: app.eqGains,
                    onEnabledChanged: controller.setEqEnabled,
                    onBandGainChanged: controller.setEqBandGain,
                    onPresetChanged: (preset) => _applyPreset(controller, preset),
                    onReset: controller.resetEq,
                  ),
                ),
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


class _GlassWrap extends StatelessWidget {
  const _GlassWrap({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final brightness = Theme.of(context).brightness;
    return Container(
      decoration: BoxDecoration(
        color: brightness == Brightness.dark
            ? Colors.white.withValues(alpha: 0.04)
            : Colors.black.withValues(alpha: 0.02),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: brightness == Brightness.dark
              ? Colors.white.withValues(alpha: 0.08)
              : Colors.black.withValues(alpha: 0.06),
          width: 0.8,
        ),
      ),
      child: child,
    );
  }
}
