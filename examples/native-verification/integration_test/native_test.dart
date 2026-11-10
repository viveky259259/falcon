import 'package:falcon_native_probe/main.dart' as app;
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('native platform channel and visible user action', (tester) async {
    app.main();
    await tester.pumpAndSettle();
    expect(find.text('Count: 0'), findsOneWidget);
    await tester.tap(find.byTooltip('Increment'));
    await tester.pumpAndSettle();
    expect(find.text('Count: 1'), findsOneWidget);

    // Query only a native capability bit. Never read or change clipboard text.
    // No mocks are registered; a missing native implementation fails the test.
    final result = await SystemChannels.platform.invokeMapMethod<String, Object?>(
      'Clipboard.hasStrings',
    );
    expect(result, isNotNull);
    expect(result!['value'], isA<bool>());
  });
}
