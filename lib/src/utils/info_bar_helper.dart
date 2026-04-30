import 'package:fluent_ui/fluent_ui.dart';

/// Shows a temporary notification bar using fluent_ui's InfoBar.
/// Replaces Material's ScaffoldMessenger.showSnackBar.
void showInfoMessage(
  BuildContext context,
  String message, {
  bool isError = false,
  Duration duration = const Duration(seconds: 3),
}) {
  displayInfoBar(
    context,
    duration: duration,
    builder: (context, close) => InfoBar(
      title: Text(message),
      severity: isError ? InfoBarSeverity.error : InfoBarSeverity.info,
    ),
  );
}
