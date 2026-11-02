import 'package:family_ai_app/main.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('shows the placeholder text', (tester) async {
    await tester.pumpWidget(const FamilyAiApp());

    expect(find.text('family-ai is coming soon'), findsOneWidget);
  });
}
