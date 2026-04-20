import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bayin_flutter/src/pages/player/player_page.dart';

void main() {
  testWidgets('PlayerPage stub renders', (WidgetTester tester) async {
    await tester.pumpWidget(
      const ProviderScope(
        child: MaterialApp(home: PlayerPage()),
      ),
    );

    expect(find.text('Player Page (stub)'), findsOneWidget);
  });
}
