// R13: options are never dropped. A save option goes only where the engine
// honours it: the editor doors take PdfSave, and the builder doors take a
// `bool deflate`, because the engine's builder save reads that one flag.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';

/// Doors and the type of their save parameter, by name.
const saveParameters = <String, String>{
  'Pdf.rewrite': 'PdfSave',
  'Pdf.editOnce': 'PdfSave',
  'Pdf.resave': 'PdfSave',
  'Pdf.merge': 'PdfSave',
  'PdfEditorHandle.save': 'PdfSave',
  'Pdf.buildTo': 'bool',
  'Pdf.imagesToPdf': 'bool',
  'PdfBuilderHandle.save': 'bool',
};

/// Doors of [sigs] whose save parameter is missing or of another type.
List<String> saveProblems(List<Sig> sigs, Map<String, String> expected) => [
  for (final MapEntry(key: label, value: type) in expected.entries)
    if (!sigs.any(
      (s) =>
          s.label == label &&
          s.params.any(
            (p) =>
                p.type == type &&
                p.name == (type == 'bool' ? 'deflate' : 'save'),
          ),
    ))
      '$label has no ${type == 'bool' ? 'bool deflate' : 'PdfSave save'}',
];

void main() {
  late List<Sig> sigs;
  setUpAll(() async => sigs = signaturesOf(await ApiModel.load()));

  test('every save door takes the option the engine honours', () {
    expect(saveProblems(sigs, saveParameters), isEmpty);
  });

  test('no builder door takes a PdfSave and no editor door a bare deflate', () {
    final stray = [
      for (final s in sigs)
        for (final p in s.params)
          if ((s.owner.startsWith('PdfBuilder') && p.type == 'PdfSave') ||
              ((s.owner == 'PdfEditorHandle' || s.owner == 'PdfEdits') &&
                  p.name == 'deflate'))
            s.label,
    ];
    expect(stray, isEmpty);
  });

  test('the wall sees a door that dropped its option', () {
    expect(
      saveProblems(
        const [Sig('Pdf', 'merge', 'method')],
        {'Pdf.merge': 'PdfSave'},
      ),
      isNotEmpty,
    );
  });
}
