import 'package:family_ai_app/main.dart' as app;
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the app starts and shows the placeholder text', (tester) async {
    app.main();
    await tester.pumpAndSettle();

    expect(find.text('family-ai is coming soon'), findsOneWidget);
  });
}
