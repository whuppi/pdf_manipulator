// No `void write(` on a DataSink implementer except MemorySink. A sink whose
// write returns `void` cannot be awaited: the bytes it takes land after the
// operation reports done (the bug an unawaited wrapper write once caused).
// Every other sink returns `FutureOr<void>` or `Future<void>`, so the
// transport awaits each write.
//
// io-exempt: the source-scan wall reads lib, test and example from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _voidWrite = RegExp(r'\bvoid\s+write\s*\(');

/// `file:line` of each `void write(` in [files] (name to source).
List<String> voidWrites(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final m in _voidWrite.allMatches(stripComments(raw)))
      '$name:${lineOf(raw, m.start)}',
];

Map<String, String> _sources() => {
  for (final dir in const [
    'lib',
    'test',
    'example/lib',
    'example/integration_test',
  ])
    for (final f in Directory(dir).listSync(recursive: true).whereType<File>())
      if (f.path.endsWith('.dart') &&
          !f.path.endsWith('lib/src/io/memory_sink.dart') &&
          !f.path.startsWith('test/guards/'))
        f.path: f.readAsStringSync(),
};

void main() {
  test('only MemorySink writes with a void return', () {
    expect(voidWrites(_sources()), isEmpty);
  });

  test('the wall sees a void write', () {
    expect(
      voidWrites({
        'x.dart':
            'class S implements DataSink {\n'
            '  @override\n'
            '  void write(Uint8List chunk) {}\n'
            '}\n',
      }),
      ['x.dart:3'],
    );
  });

  test('a FutureOr write passes', () {
    expect(
      voidWrites({
        'x.dart':
            'class S implements DataSink {\n'
            '  FutureOr<void> write(Uint8List chunk) {}\n'
            '}\n',
      }),
      isEmpty,
    );
  });
}
