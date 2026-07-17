// io-exempt: builds a throwaway fixture app on disk — the detector's
// contract IS filesystem analysis, so this test cannot be pure.
//
// The detector's one silent failure mode is a keep-set that misses a
// capability the app reaches: `trim: auto` would then strip code the
// app calls, and the user meets the typed not-enabled error at runtime.
// This test pins the two resolution shapes against a REAL resolved
// fixture app:
//   - extension members (sign / convertTo / compress live in
//     `extension ... on Pdf`) record under their BARE name
//   - class members (PdfDoc.extract) record under `Class.member`
// If the analyzer's element model ever changes either shape, this fails
// before any consumer's build lies.

import 'dart:convert';
import 'dart:io';

import 'package:test/test.dart';

import 'package:pdf_manipulator/src/trim/capabilities.dart';
import 'package:pdf_manipulator/src/trim/detector.dart';

const _fixtureMain = '''
import 'package:pdf_manipulator/pdf_manipulator.dart';

Future<void> use(Pdf pdf, DataSource src, DataSink out) async {
  // Extension members — one per capability their extension records.
  await pdf.compress(src, out);                                  // render
  await pdf.sign(src, out,
      credentials: const PdfSigningCredentials.pem('c', 'k'));   // signatures
  await pdf.convertTo(src, out, format: PdfDocumentFormat.docx); // office

  // Class members — the qualified Class.member shape.
  final doc = await pdf.open(src);
  await doc.extract(pages: const PdfPages.all());                // extract
  await doc.dispose();
  final e = await pdf.edit(src);
  await e.convertToPdfA();                                       // pdfa
  await e.dispose();
}
''';

void main() {
  late Directory fixture;

  setUpAll(() {
    fixture = Directory.systemTemp.createTempSync('trim_detector_fixture_');
    File('${fixture.path}/pubspec.yaml').writeAsStringSync(
      'name: trim_detector_fixture\n'
      'environment:\n'
      "  sdk: '>=3.10.0 <4.0.0'\n",
    );
    Directory('${fixture.path}/lib').createSync();
    File('${fixture.path}/lib/main.dart').writeAsStringSync(_fixtureMain);

    // The fixture resolves against THIS package's dependency set: reuse
    // the root package_config with every rootUri made absolute, plus an
    // entry for the fixture itself. No pub get, no network.
    final rootConfigFile = File('.dart_tool/package_config.json');
    final rootConfig =
        jsonDecode(rootConfigFile.readAsStringSync()) as Map<String, Object?>;
    final configDir = rootConfigFile.absolute.parent.uri;
    final packages = [
      for (final p in rootConfig['packages'] as List)
        {
          ...p as Map<String, Object?>,
          'rootUri': configDir.resolve(p['rootUri'] as String).toString(),
        },
      {
        'name': 'trim_detector_fixture',
        'rootUri': fixture.absolute.uri.toString(),
        'packageUri': 'lib/',
      },
    ];
    Directory('${fixture.path}/.dart_tool').createSync();
    File(
      '${fixture.path}/.dart_tool/package_config.json',
    ).writeAsStringSync(jsonEncode({...rootConfig, 'packages': packages}));
  });

  tearDownAll(() {
    fixture.deleteSync(recursive: true);
  });

  test('detector sees extension members AND class members', () async {
    final result = await detectCapabilities(fixture.path);

    expect(
      result.unresolvedPaths,
      isEmpty,
      reason: 'the fixture must fully resolve or the scan proves nothing',
    );
    expect(result.resolved, isTrue);

    expect(
      result.keep,
      PdfCapability.values.toSet(),
      reason:
          'a missing capability here means trim: auto would strip '
          'code the app calls — the runtime typed error a consumer '
          'was promised never to see',
    );

    // Pin the key shapes, not just the outcome: extensions record bare,
    // classes record qualified. apiMembers carries both on purpose.
    expect(
      result.matchedMembers,
      containsAll(<String>{
        'compress', 'sign', 'convertTo', // extension → bare
        'PdfDoc.extract', 'PdfEditor.convertToPdfA', // class → qualified
      }),
    );
  });
}
