import 'package:flutter/material.dart';

void main() {
  runApp(const FamilyAiApp());
}

class FamilyAiApp extends StatelessWidget {
  const FamilyAiApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      title: 'family-ai',
      home: Scaffold(body: Center(child: Text('family-ai is coming soon'))),
    );
  }
}
