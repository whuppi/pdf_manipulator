// The trimmed shell's ENTIRE app code: run the real example app. Being
// launched from THIS package root is what makes it trimmed — the
// pubspec's trim user_define makes the build hook compile the engine
// with only the render capability, and every other capability's ops
// answer the typed not-enabled error at runtime. See ../pubspec.yaml
// and example/lib/main.dart.

import 'package:pdf_manipulator_example/main.dart' as app;

void main() => app.main();
