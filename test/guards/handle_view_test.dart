// R7: a scope body receives a view. `read`, `rewrite` and `buildTo` type their
// body on PdfDocument, PdfEditor and PdfBuilder; those three declare no `save`
// or `dispose`; only the *Handle classes do.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';

/// The body parameter type each scope door must take.
const scopeBodies = <String, String>{
  'Pdf.read': 'Future<T> Function(PdfDocument)',
  'Pdf.rewrite': 'Future<void> Function(PdfEditor)',
  'Pdf.buildTo': 'Future<void> Function(PdfBuilder)',
};

/// Problems with the view/handle split in [sigs].
List<String> viewProblems(List<Sig> sigs) {
  final problems = <String>[];
  for (final MapEntry(key: label, value: body) in scopeBodies.entries) {
    final door = sigs.where((s) => s.label == label);
    if (door.isEmpty || !door.first.params.any((p) => p.type == body)) {
      problems.add('$label body is not $body');
    }
  }
  for (final s in sigs) {
    final view = const {
      'PdfDocument',
      'PdfEditor',
      'PdfBuilder',
    }.contains(s.owner);
    if (view && (s.name == 'save' || s.name == 'dispose')) {
      problems.add('${s.label} belongs on the handle');
    }
  }
  for (final handle in const [
    'PdfDocumentHandle',
    'PdfEditorHandle',
    'PdfBuilderHandle',
  ]) {
    if (!sigs.any((s) => s.owner == handle && s.name == 'dispose')) {
      problems.add('$handle has no dispose');
    }
  }
  for (final handle in const ['PdfEditorHandle', 'PdfBuilderHandle']) {
    if (!sigs.any((s) => s.owner == handle && s.name == 'save')) {
      problems.add('$handle has no save');
    }
  }
  return problems;
}

void main() {
  late List<Sig> sigs;
  setUpAll(() async => sigs = signaturesOf(await ApiModel.load()));

  test('scope bodies get views and only handles save and dispose', () {
    expect(viewProblems(sigs), isEmpty);
  });

  test('the wall sees a view with a dispose', () {
    expect(
      viewProblems([...sigs, const Sig('PdfEditor', 'dispose', 'method')]),
      contains('PdfEditor.dispose belongs on the handle'),
    );
  });
}
