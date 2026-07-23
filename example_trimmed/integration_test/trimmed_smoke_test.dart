// The trimmed contract, asserted end to end on a real trimmed engine:
//   1. core ops work (parse, build, edit, forms)
//   2. the KEPT capability works (render)
//   3. every EXCLUDED capability answers the typed not-enabled error —
//      never a crash, never a silent no-op
//
// This is the only suite that runs against a trimmed binary in CI; the
// full example's suites cover the full binary. Keep the keep-list here
// in lockstep with ../pubspec.yaml's `keep:` user_define.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

/// An operation excluded from this build must answer a [PdfError] that
/// says so — the fail-closed contract consumers rely on.
final Matcher throwsNotEnabled = throwsA(
  isA<PdfError>().having(
    (e) => e.message.toLowerCase(),
    'message',
    contains('not enabled'),
  ),
);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  final pdf = Pdf();
  late Uint8List onePage;

  setUpAll(() async {
    final b = await pdf.build();
    final page = await b.addA4Page();
    await page.text('trimmed build smoke');
    await page.textField(
      'note',
      const PdfRect(x: 100, y: 680, width: 200, height: 20),
    );
    await page.done();
    final sink = MemorySink();
    await b.save(sink);
    await b.dispose();
    onePage = sink.takeBytes();
  });

  tearDownAll(() async {
    await pdf.dispose();
  });

  testWidgets('core works: open, edit, fill, flatten', (t) async {
    final doc = await pdf.open(MemorySource(onePage));
    expect(doc.pageCount, 1);
    await doc.dispose();

    final e = await pdf.edit(MemorySource(onePage));
    await e.setFormFieldValue('note', 'still here');
    await e.flattenForms();
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('kept capability works: render', (t) async {
    final doc = await pdf.open(MemorySource(onePage));
    var pages = 0;
    await for (final page in doc.render(pages: const PdfPages.single(0))) {
      expect(page.data, isNotEmpty);
      pages++;
    }
    expect(pages, 1);
    await doc.dispose();
  });

  testWidgets('excluded: extract answers the typed error', (t) async {
    final doc = await pdf.open(MemorySource(onePage));
    await expectLater(
      () => doc.extract(pages: const PdfPages.single(0)),
      throwsNotEnabled,
    );
    await doc.dispose();
  });

  testWidgets('excluded: pdfa validation answers the typed error', (t) async {
    final doc = await pdf.open(MemorySource(onePage));
    await expectLater(() => doc.validatePdfA(), throwsNotEnabled);
    await doc.dispose();
  });

  testWidgets('excluded: office conversion answers the typed error', (t) async {
    final sink = MemorySink();
    await expectLater(
      () => pdf.convertTo(
        MemorySource(onePage),
        sink,
        format: PdfDocumentFormat.docx,
      ),
      throwsNotEnabled,
    );
  });
}
