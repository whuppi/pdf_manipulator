// Runs all shared ops through the public Pdf API (native path).
// The SAME tests run through WebBridge in web_runner_test.dart.

@TestOn('!browser')
library;

import 'package:pdf_manipulator/pdf_manipulator.dart';
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
import 'timing.dart';

void main() {
  late Pdf pdf;

  setUpAll(() => pdf = Pdf());
  tearDownAll(() => pdf.dispose());

  registerOpenTests(() => pdf);
  registerMergeTests(() => pdf);
  registerStructuralTests(() => pdf);
  registerContentTests(() => pdf);
  registerStreamTests(() => pdf);
  registerSecurityTests(() => pdf);
  registerEditorTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerErrorTests(() => pdf);
  registerLifecycleTests(() => Pdf());
  registerTimingTests(() => pdf);
  registerStressTests(() => pdf);
  registerCoverageCheck();
}
