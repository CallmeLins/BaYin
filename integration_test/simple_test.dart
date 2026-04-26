import 'package:bayin_flutter/main.dart';
import 'package:bayin_flutter/src/i18n/strings.g.dart';
import 'package:bayin_flutter/src/rust/rust_api.dart';
import 'package:bayin_flutter/src/services/library_service.dart';
import 'package:bayin_flutter/src/services/settings_service.dart';
import 'package:bayin_flutter/src/services/window_service.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await SettingsService.init();
    await WindowService.init();
    await RustApi.instance.ensureInitialized();
    await LibraryService.instance.ensureDatabaseInitialized();
    LocaleSettings.useDeviceLocale();
  });

  testWidgets('app boots and renders MaterialApp shell',
      (WidgetTester tester) async {
    await tester.pumpWidget(
      TranslationProvider(child: const ProviderScope(child: BaYinApp())),
    );
    await tester.pumpAndSettle(const Duration(seconds: 2));

    expect(find.byType(MaterialApp), findsOneWidget);
  });

  testWidgets('Rust FFI ping still works', (WidgetTester tester) async {
    expect(RustApi.instance.ping(), 'pong from Rust');
  });
}
