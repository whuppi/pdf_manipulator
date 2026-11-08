// R1: a door that opens a PDF takes PdfInput, and nothing else names a
// password. No public signature carries `String? password` or
// `Map<DataSource`, and a parameter of type PdfInput appears only in a door
// that opens a PDF.
//
// io-exempt: the analyzer and the source scan read lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'api_model.dart';
import 'source_scan.dart';

/// The public members that take a PdfInput: the doors that open a PDF.
const doors = <String>{
  'Pdf.open',
  'Pdf.edit',
  'Pdf.read',
  'Pdf.rewrite',
  'Pdf.readOnce',
  'Pdf.editOnce',
  'Pdf.info',
  'Pdf.resave',
  'Pdf.merge',
  'Pdf.split',
  'Pdf.compress',
  'Pdf.sign',
  'Pdf.convertToOffice',
  'Pdf.convertXfaToAcroForm',
  'PdfEditor.appendPages',
};

/// Members of [sigs] that take a PdfInput (alone or in a list) and are not
/// in [allowed].
List<String> strayInputs(List<Sig> sigs, Set<String> allowed) => [
  for (final s in sigs)
    if (s.kind == 'method' &&
        s.params.any((p) => p.type.contains('PdfInput')) &&
        !allowed.contains(s.label))
      s.label,
];

/// `file:line` of each `String? password` or `Map<DataSource` in [files].
List<String> passwordSpellings(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final m in RegExp(
      r'String\? password|Map<DataSource',
    ).allMatches(stripComments(raw)))
      '$name:${lineOf(raw, m.start)}',
];

/// Owners that may name a password: the input that carries it, and the
/// signing bundle, whose password unlocks a certificate, not a document.
const passwordOwners = {'PdfInput', 'PdfFile', 'PdfSigningCredentials'};

void main() {
  late List<Sig> sigs;
  setUpAll(() async => sigs = signaturesOf(await ApiModel.load()));

  test('no public signature names a password but PdfInput', () {
    final named = [
      for (final s in sigs)
        for (final p in s.params)
          if (p.name == 'password' && !passwordOwners.contains(s.owner))
            s.label,
    ];
    expect(named, isEmpty);
  });

  test('PdfInput appears only in the doors that open a PDF', () {
    expect(strayInputs(sigs, doors), isEmpty);
  });

  test('every door is a public member that exists', () {
    final labels = {for (final s in sigs) s.label};
    expect(doors.difference(labels), isEmpty);
  });

  test('no `String? password` or `Map<DataSource` in lib/src/ops', () {
    final files = {
      for (final f in Directory('lib/src/ops').listSync().whereType<File>())
        if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
    };
    expect(passwordSpellings(files), isEmpty);
  });

  group('the wall sees a violation', () {
    test('a stray PdfInput', () {
      expect(
        strayInputs(const [
          Sig('Pdf', 'zip', 'method', params: [Par('input', 'PdfInput')]),
        ], doors),
        ['Pdf.zip'],
      );
    });

    test('a password spelling', () {
      expect(passwordSpellings({'x.dart': 'f({String? password}) {}\n'}), [
        'x.dart:1',
      ]);
      expect(
        passwordSpellings({'x.dart': 'f(Map<DataSource, String> p) {}\n'}),
        ['x.dart:1'],
      );
    });
  });
}
