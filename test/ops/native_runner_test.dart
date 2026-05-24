// Runs all shared ops through NativeBridge (phone/desktop path).
// The SAME tests run through WebBridge in web_runner_test.dart.

@TestOn('!browser')
library;

import 'package:pdf_manipulator/src/transport/native/bridge.dart';
import 'package:test/test.dart';

import 'open.dart';
import 'merge.dart';
import 'structural.dart';
import 'content.dart';
import 'stream.dart';
import 'security.dart';
import 'editor.dart';
import 'builder.dart';
import 'error.dart';
import 'lifecycle.dart';
import 'coverage_check.dart';
import 'stress.dart';

void main() {
  late NativeBridge bridge;

  setUpAll(() => bridge = NativeBridge());
  tearDownAll(() => bridge.dispose());

  registerOpenTests(() => bridge);
  registerMergeTests(() => bridge);
  registerStructuralTests(() => bridge);
  registerContentTests(() => bridge);
  registerStreamTests(() => bridge);
  registerSecurityTests(() => bridge);
  registerEditorTests(() => bridge);
  registerBuilderTests(() => bridge);
  registerErrorTests(() => bridge);
  registerLifecycleTests(() => NativeBridge());
  registerStressTests(() => bridge);
  registerCoverageCheck();
}
