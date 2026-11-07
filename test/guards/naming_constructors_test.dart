// N7: a named constructor names the value it makes; never `create`, `new`,
// `make`, `build`, `config`, `set`.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';

const _banned = {'create', 'new', 'make', 'build', 'config', 'set'};

/// The named constructors of [sigs] with a banned name.
List<String> constructorProblems(List<Sig> sigs) => [
  for (final s in sigs)
    if (s.kind == 'constructor' && s.name != 'new' && _banned.contains(s.name))
      s.label,
];

void main() {
  test('no named constructor uses a banned name', () async {
    expect(constructorProblems(signaturesOf(await ApiModel.load())), isEmpty);
  });

  test('the wall sees a banned constructor', () {
    expect(constructorProblems(const [Sig('X', 'config', 'constructor')]), [
      'X.config',
    ]);
    expect(
      constructorProblems(const [Sig('X', 'password', 'constructor')]),
      isEmpty,
    );
  });
}
