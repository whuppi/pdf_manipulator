// io-exempt: builds a throwaway fixture app on disk — the detector's
// contract IS filesystem scanning, so this test cannot be pure.
//
// The detector's one dangerous failure mode is a keep-set that misses a
// capability the app reaches: `keep: auto` would then strip code the
// app calls, and the user meets the typed not-enabled error at runtime.
// The text scan's contract: any spelled member name in a file that
// imports pdf_manipulator keeps its capability (over-keep is allowed,
// under-keep is not), and files that never import the package are
// ignored entirely.

import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:test/test.dart';

import 'package:pdf_manipulator/src/keep/capabilities.dart';
import 'package:pdf_manipulator/src/keep/detector.dart';

const _usesEverything = '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> use(Pdf pdf, DataSource src, DataSink out) async {
  await pdf.compress(src, out);                                  // render
  await pdf.sign(src, out,
      credentials: const PdfSigningCredentials.pem('c', 'k'));   // signatures
  await pdf.convertTo(src, out, format: PdfDocumentFormat.docx); // office
  final doc = await pdf.open(src);
  await doc.extract(pages: const PdfPages.all());                // extract
  await doc.dispose();
  final e = await pdf.edit(src);
  await e.convertToPdfA();                                       // pdfa
  await e.dispose();
}
''';

// Spells capability member names, but never imports pdf_manipulator —
// these are the app's own identifiers and must keep nothing.
const _unrelated = '''
class Reporter {
  void sign(String name) {}
  void render(Object canvas) {}
  String extract(String raw) => raw;
}
''';

const _coreOnly = '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> use(Pdf pdf, DataSource a, DataSource b, DataSink out) =>
    pdf.merge([a, b], out);
''';

Directory _fixture(Map<String, String> files) {
  final dir = Directory.systemTemp.createTempSync('trim_detector_fixture_');
  File('${dir.path}/pubspec.yaml').writeAsStringSync('name: fixture_app\n');
  for (final e in files.entries) {
    File('${dir.path}/lib/${e.key}')
      ..parent.createSync(recursive: true)
      ..writeAsStringSync(e.value);
  }
  return dir;
}

void main() {
  test('every capability is found, through any call shape', () async {
    final dir = _fixture({'main.dart': _usesEverything});
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      PdfCapability.values.toSet(),
      reason:
          'a missing capability here means keep: auto would strip '
          'code the app calls — the runtime typed error a consumer '
          'was promised never to see',
    );
    expect(
      result.matchedMembers,
      containsAll(<String>{'compress', 'sign', 'convertTo', 'extract'}),
    );
  });

  test('files that do not import pdf_manipulator keep nothing', () async {
    final dir = _fixture({'reporter.dart': _unrelated});
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      isEmpty,
      reason:
          'same-named identifiers outside importing files must not '
          'inflate the keep-set',
    );
  });

  test('core-only app trims every capability', () async {
    final dir = _fixture({'main.dart': _coreOnly, 'reporter.dart': _unrelated});
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(result.keep, isEmpty);
  });

  test('usage through a re-export barrel is detected', () async {
    final dir = _fixture({
      'deps/barrel.dart':
          "export 'package:pdf_manipulator/"
          "pdf_manipulator.dart';\n",
      // Relative import of the barrel — no direct pdf_manipulator import.
      'sign_flow.dart': '''
import 'deps/barrel.dart';

Future<void> use(Pdf pdf, DataSource src, DataSink out) => pdf.sign(src, out,
    credentials: const PdfSigningCredentials.pem('c', 'k'));
''',
      // Second hop: a barrel re-exporting the barrel, imported by name.
      'deps/deps.dart': "export 'barrel.dart';\n",
      'render_flow.dart': '''
import 'package:fixture_app/deps/deps.dart';

Stream<RenderedPage> use(PdfDoc doc) =>
    doc.render(pages: const PdfPages.all());
''',
    });
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      containsAll({PdfCapability.signatures, PdfCapability.render}),
      reason:
          'barrel-mediated usage must be kept — under-keeping strips '
          'code the app calls',
    );
  });

  test('conditional imports track every branch URI', () async {
    final dir = _fixture({
      'stub.dart': '// inert stub for the non-io branch\n',
      'deps/barrel.dart':
          "export 'package:pdf_manipulator/pdf_manipulator.dart';\n",
      // Only the second (conditional) URI reaches the API.
      'sign_flow.dart': '''
import 'stub.dart' if (dart.library.io) 'deps/barrel.dart';

Future<void> use(Pdf pdf, DataSource src, DataSink out) => pdf.sign(src, out,
    credentials: const PdfSigningCredentials.pem('c', 'k'));
''',
    });
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      contains(PdfCapability.signatures),
      reason:
          'a conditional import branch is a real path to the API — '
          'dropping it under-keeps',
    );
  });

  test('quoted pubspec name still resolves self-imports', () async {
    final dir = _fixture({
      'deps/barrel.dart':
          "export 'package:pdf_manipulator/pdf_manipulator.dart';\n",
      'sign_flow.dart': '''
import 'package:fixture_app/deps/barrel.dart';

Future<void> use(Pdf pdf, DataSource src, DataSink out) => pdf.sign(src, out,
    credentials: const PdfSigningCredentials.pem('c', 'k'));
''',
    });
    File('${dir.path}/pubspec.yaml').writeAsStringSync('name: "fixture_app"\n');
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      contains(PdfCapability.signatures),
      reason:
          'YAML quotes around the name are syntax, not part of the '
          'package name — dropping them must not lose the barrel chain',
    );
  });

  test('member names in comments do not keep capabilities', () async {
    final dir = _fixture({
      // The issue-#175 shape: prose mentioning `render` next to a real
      // merge call — only merge's capability set may be kept.
      'main.dart': '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

// Merge the pages, then render them.
/// Later we could render thumbnails here.
/* A block note: sign and extract are not used.
   /* nested: convertToPdfA */ still inside: convertTo */
Future<void> use(Pdf pdf, DataSource a, DataSource b, DataSink out) =>
    pdf.merge([a, b], out);
''',
    });
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      isEmpty,
      reason:
          'comments are prose, not call sites — a doc line mentioning '
          'render must not keep the render capability',
    );
  });

  test('string contents still match — interpolated calls are real', () async {
    final dir = _fixture({
      'main.dart': '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

String use(PdfDoc doc) => 'pages: \${doc.render(pages: const PdfPages.all())}';
''',
    });
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      contains(PdfCapability.render),
      reason:
          'a call inside a string interpolation is a real call site — '
          'blanking strings would under-keep it',
    );
  });

  test('// inside a string literal is not a comment', () async {
    final dir = _fixture({
      // If string tracking failed, the URL's `//` would blank the rest
      // of the line — including the real extract call after it.
      'main.dart': '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> use(PdfDoc doc) async { const u = 'http://x'; await doc.extract(pages: const PdfPages.all()); }
''',
    });
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(
      result.keep,
      contains(PdfCapability.extract),
      reason:
          'treating the // inside a string as a comment would blank a '
          'real call on the same line — an under-keep',
    );
  });

  test('match sites point at the file and line of the first match', () async {
    final dir = _fixture({
      'main.dart': '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

// A comment mentioning sign must not become the reported site.
Future<void> use(Pdf pdf, DataSource src, DataSink out) => pdf.sign(src, out,
    credentials: const PdfSigningCredentials.pem('c', 'k'));
''',
    });
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(result.resolved, isTrue);
    expect(result.matchSites['sign'], '${p.join('lib', 'main.dart')}:4');
  });

  test('missing pubspec fails closed — self-imports cannot resolve', () async {
    final dir = _fixture({'main.dart': _usesEverything});
    File('${dir.path}/pubspec.yaml').deleteSync();
    addTearDown(() => dir.deleteSync(recursive: true));

    final result = detectCapabilities(dir.path);
    expect(
      result.resolved,
      isFalse,
      reason:
          'without the app name, package:<self>/ barrel imports '
          'cannot be tracked — the caller must keep the full binary',
    );
    expect(result.unresolvedPaths, contains(endsWith('pubspec.yaml')));
  });
}
