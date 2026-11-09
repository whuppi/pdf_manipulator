// N5, N6 and N11: what an exported name ends with.
//
// - No `Options` or `Params` (listed rows aside) and `Config` only on
//   PdfConfig; `Result` never.
// - A `*Info` is facts about one thing: a final class that declares only final
//   fields and getters, `==`, `hashCode` and `toString`. A `*Report` carries at
//   least one count.
// - `Type` never; `Subtype` only PdfAnnotationSubtype; `Format` and
//   `Algorithm` are enums.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:test/test.dart';

import 'api_model.dart';
import 'naming_exceptions.dart';

/// Problems with the suffixes of the exported [types].
List<String> suffixProblems(Map<String, Element> types, ApiModel? model) {
  final problems = <String>[];
  for (final MapEntry(key: name, value: element) in types.entries) {
    if (name.endsWith('Options') && !optionsNames.containsKey(name)) {
      problems.add('$name: no Options');
    }
    if (name.endsWith('Params')) problems.add('$name: no Params');
    if (name.endsWith('Config') && name != 'PdfConfig') {
      problems.add('$name: Config is PdfConfig only');
    }
    if (name.endsWith('Result')) problems.add('$name: no Result');
    if (name.endsWith('Type')) problems.add('$name: Type is Kind');
    if (name.endsWith('Subtype') && name != 'PdfAnnotationSubtype') {
      problems.add('$name: one Subtype');
    }
    if ((name.endsWith('Format') || name.endsWith('Algorithm')) &&
        element is! EnumElement) {
      problems.add('$name: a Format or Algorithm is an enum');
    }
    if (model != null && element is ClassElement) {
      final members = model.membersOf(element).toList();
      if (name.endsWith('Info')) {
        if (!element.isFinal) problems.add('$name: an Info is final');
        for (final m in members) {
          final e = m.element;
          if (e is FieldElement && !e.isFinal && !e.isStatic) {
            problems.add('$name.${m.name}: Info fields are final');
          }
          if (e is MethodElement &&
              !const {'==', 'toString'}.contains(m.name)) {
            problems.add('$name.${m.name}: an Info has no methods');
          }
        }
      }
      if (name.endsWith('Report')) {
        final hasCount = members.any(
          (m) => m.typeName == 'int' && !m.isConstructor,
        );
        if (!hasCount) problems.add('$name: a Report carries a count');
      }
    }
  }
  return problems;
}

void main() {
  late ApiModel model;
  setUpAll(() async => model = await ApiModel.load());

  test('exported names follow N5, N6 and N11', () {
    expect(suffixProblems(model.all, model), isEmpty);
  });

  test('every Options row names an exported type', () {
    for (final name in optionsNames.keys) {
      expect(model.all, contains(name));
    }
  });

  test('the wall sees a bad suffix', () {
    final element = model.type('PdfRect');
    for (final name in [
      'PdfFooOptions',
      'PdfFooParams',
      'PdfFooConfig',
      'PdfFooResult',
      'PdfFooType',
      'PdfFooSubtype',
      'PdfFooFormat',
    ]) {
      expect(suffixProblems({name: element}, null), isNotEmpty, reason: name);
    }
    expect(suffixProblems({'PdfConfig': element}, null), isEmpty);
  });
}
