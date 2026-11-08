// N14: no abbreviations and no underscores in a public identifier; the
// acronym list is closed. A run of two or three capitals inside a CamelCase
// name must be in acronyms.dart, and the banned tokens never appear.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'acronyms.dart';
import 'api_model.dart';

const _banned = {'Doc', 'Hq', 'Pt', 'Cfg', 'Idx', 'Num', 'Str', 'Img', 'Mod'};

/// The CamelCase tokens of [name]: `PdfRc4Key40` is Pdf, Rc4, Key, 40.
List<String> tokens(String name) => RegExp(
  r'[A-Z][a-z]+|[A-Z]+[0-9]*(?![a-z])|[A-Z][a-z]*[0-9]+|[a-z]+[0-9]*|[0-9]+',
).allMatches(name).map((m) => m.group(0)!).toList();

/// Problems with [name].
List<String> tokenProblems(String name) {
  final problems = <String>[];
  if (name.contains('_')) problems.add('$name: an underscore');
  final cap = name.isEmpty
      ? name
      : '${name[0].toUpperCase()}${name.substring(1)}';
  for (final t in tokens(cap)) {
    if (_banned.contains(t)) problems.add('$name: banned token $t');
    if (RegExp(r'^[A-Z]{2,3}[0-9]*$').hasMatch(t) && !acronyms.contains(t)) {
      problems.add('$name: $t is not an acronym on the list');
    }
  }
  return problems;
}

void main() {
  test('every public identifier follows N14', () async {
    final model = await ApiModel.load();
    final names = <String>{
      ...model.all.keys,
      for (final s in signaturesOf(model)) ...[
        s.name.split('.').last,
        for (final p in s.params) p.name,
      ],
    };
    expect([
      for (final n in names)
        if (!n.startsWith('_')) ...tokenProblems(n),
    ], isEmpty);
  });

  test('every acronym on the list is camel-cased', () {
    for (final a in acronyms) {
      expect(a, matches(RegExp(r'^[A-Z][a-z0-9]*$')));
    }
  });

  test('the wall sees a violation', () {
    expect(tokenProblems('rc4_40'), isNotEmpty);
    expect(tokenProblems('printHq'), isNotEmpty);
    expect(tokenProblems('PdfDoc'), isNotEmpty);
    expect(tokenProblems('PdfXYZ'), isNotEmpty);
    expect(tokenProblems('PdfDocument'), isEmpty);
    expect(tokenProblems('PdfPkcs12Credentials'), isEmpty);
  });
}
