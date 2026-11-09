// N9: a length is in points and carries no suffix; every other number carries
// its unit. Every public numeric field and parameter name is a row in
// units.dart; a row with a suffix unit has a name that ends with it.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';
import 'units.dart';

/// Names of [sigs] (numeric fields and parameters) that break N9.
List<String> unitProblems(List<Sig> sigs, Map<String, String> table) {
  final problems = <String>{};
  void check(String label, String name, String type) {
    if (type != 'int' &&
        type != 'int?' &&
        type != 'double' &&
        type != 'double?') {
      return;
    }
    final unit = table[name];
    if (unit == null) {
      problems.add('$label: $name has no row in units.dart');
      return;
    }
    final suffix = suffixUnits[unit];
    if (suffix != null &&
        !name.endsWith(suffix) &&
        name != suffix.toLowerCase()) {
      problems.add('$name: unit $unit needs the suffix $suffix');
    }
    if (unit == 'points' && suffixUnits.values.any(name.endsWith)) {
      problems.add('$name: points carry no suffix');
    }
  }

  for (final s in sigs) {
    if ((s.kind == 'field' || s.kind == 'getter') && s.name != 'hashCode') {
      check(s.label, s.name, s.type);
    }
    for (final p in s.params) {
      check(s.label, p.name, p.type);
    }
  }
  return problems.toList()..sort();
}

void main() {
  late List<Sig> sigs;
  setUpAll(() async => sigs = signaturesOf(await ApiModel.load()));

  test('every public number is a row and carries its unit', () {
    expect(unitProblems(sigs, units), isEmpty);
  });

  test('every row names a number that exists', () {
    final used = {
      for (final s in sigs) ...[s.name, for (final p in s.params) p.name],
    };
    expect(units.keys.toSet().difference(used), isEmpty);
  });

  test('the wall sees a missing row and a missing suffix', () {
    expect(
      unitProblems(const [
        Sig('X', 'f', 'method', params: [Par('timeout', 'int')]),
      ], {}),
      hasLength(1),
    );
    expect(
      unitProblems(
        const [Sig('X', 'size', 'field', type: 'int')],
        {'size': 'bytes'},
      ),
      hasLength(1),
    );
    expect(
      unitProblems(
        const [Sig('X', 'width', 'field', type: 'double')],
        {'width': 'points'},
      ),
      isEmpty,
    );
  });
}
