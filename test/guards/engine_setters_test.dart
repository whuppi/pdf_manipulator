// No host code and no Dart calls an upstream process-wide setter. Upstream's
// bindings (python, ffi, wasm.rs) set the crypto provider and policy, the
// operator cap and the unmapped-glyph switch once for the process; the host
// has one executor per lane, and a call from it would give every lane the
// last caller's setting.
//
// io-exempt: the source-scan wall reads the host's Rust and lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _setter = RegExp(
  r'\b(set_provider|set_policy|set_max_ops_per_stream|set_preserve_unmapped_glyphs)\b',
);

/// `file:line: name` of each setter named in [files] (name to source),
/// outside comments.
List<String> setterCalls(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final m in _setter.allMatches(stripComments(raw)))
      '$name:${lineOf(raw, m.start)}: ${m.group(1)}',
];

Map<String, String> _sources() => {
  for (final (dir, extension) in const [
    ('vendor/pdf_oxide/src/host', '.rs'),
    ('lib', '.dart'),
  ])
    for (final f in Directory(dir).listSync(recursive: true).whereType<File>())
      if (f.path.endsWith(extension)) f.path: f.readAsStringSync(),
};

void main() {
  test('the wall sees a setter call and ignores one in a comment', () {
    expect(
      setterCalls({
        'a.rs': 'fn f() {\n    crypto::set_policy(p);\n}\n',
        'b.rs': '// set_provider is upstream\nfn g() {}\n',
      }),
      ['a.rs:2: set_policy'],
    );
  });

  test('no host file and nothing in lib calls an upstream setter', () {
    final sources = _sources();
    expect(sources, isNotEmpty);
    expect(setterCalls(sources), isEmpty);
  });
}
