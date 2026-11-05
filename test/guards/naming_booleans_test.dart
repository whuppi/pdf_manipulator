// N8: a public bool field, getter or named parameter reads as a predicate
// (`is`, `has`, `can`, `requires`, `covers`, `allow`) or, for a save or policy
// knob, as an imperative listed in naming_exceptions.dart.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';
import 'naming_exceptions.dart';

final _predicate = RegExp(r'^(is|has|can|requires|covers|allow)[A-Z]');

/// The bool names of [sigs] that are not predicates and not listed.
List<String> booleanProblems(List<Sig> sigs) => {
  for (final s in sigs)
    if ((s.kind == 'field' || s.kind == 'getter') && s.type == 'bool')
      if (!_predicate.hasMatch(s.name) && !boolVerbs.containsKey(s.name))
        s.label,
  for (final s in sigs)
    for (final p in s.params)
      if (p.isNamed && (p.type == 'bool' || p.type == 'bool?'))
        if (!_predicate.hasMatch(p.name) && !boolVerbs.containsKey(p.name))
          '${s.label}(${p.name})',
}.toList();

void main() {
  late List<Sig> sigs;
  setUpAll(() async => sigs = signaturesOf(await ApiModel.load()));

  test('every public bool reads as a predicate or a listed knob', () {
    expect(booleanProblems(sigs), isEmpty);
  });

  test('every listed knob is still used', () {
    final used = {
      for (final s in sigs) ...[
        if (s.type == 'bool') s.name,
        for (final p in s.params)
          if (p.type == 'bool' || p.type == 'bool?') p.name,
      ],
    };
    expect(boolVerbs.keys.toSet().difference(used), isEmpty);
  });

  test('the wall sees a bad bool', () {
    expect(
      booleanProblems(const [Sig('X', 'readOnly', 'field', type: 'bool')]),
      ['X.readOnly'],
    );
    expect(
      booleanProblems(const [
        Sig(
          'X',
          'f',
          'method',
          params: [Par('checked', 'bool', isNamed: true)],
        ),
      ]),
      ['X.f(checked)'],
    );
    expect(
      booleanProblems(const [Sig('X', 'isReadOnly', 'field', type: 'bool')]),
      isEmpty,
    );
  });
}
