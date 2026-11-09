// The Asyncify instrumentation list and its wall. A hand-assembled module
// stands in for the engine: two pausing imports, a direct caller, a function
// that reaches the import only through a table, and functions that never do.

@TestOn('vm')
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/hook/asyncify_list.dart';
import 'package:test/test.dart';

List<int> _u32(int v) {
  final out = <int>[];
  do {
    var b = v & 0x7f;
    v >>= 7;
    if (v != 0) b |= 0x80;
    out.add(b);
  } while (v != 0);
  return out;
}

List<int> _name(String s) => [..._u32(s.length), ...utf8.encode(s)];

List<int> _section(int id, List<int> body) => [
  id,
  ..._u32(body.length),
  ...body,
];

List<int> _body(List<int> code) => [..._u32(code.length + 1), 0, ...code];

/// Function indices: 0 and 1 are the imports; 2 calls import 0; 3 calls 2;
/// 4 calls index 5-or-7's type through the table; 5 (in the table) calls 2;
/// 6 calls nothing; 7 (in the table) calls nothing. The list names a defined
/// function by its index among the defined ones (index minus the 2 imports).
/// Table 0 holds functions; table 1, as wasm-bindgen's externref table, holds
/// host values.
Uint8List _module({List<int>? extra}) {
  const t0 = [0x60, 0, 0];
  const t1 = [0x60, 1, 0x7f, 0];
  return Uint8List.fromList([
    0,
    0x61,
    0x73,
    0x6d,
    1,
    0,
    0,
    0,
    ..._section(1, [2, ...t0, ...t1]),
    ..._section(2, [
      2,
      ..._name('./g.js'),
      ..._name('__wbg_host_read_at_1'),
      0,
      0,
      ..._name('./g.js'),
      ..._name('__wbg_host_write_chunk_2'),
      0,
      0,
    ]),
    ..._section(3, [6, 0, 0, 0, 1, 0, 1]),
    ..._section(4, [2, 0x70, 0, 2, 0x6f, 0, 0]),
    ..._section(9, [1, 0, 0x41, 0, 0x0b, 2, 5, 7]),
    ..._section(10, [
      6,
      ..._body([0x10, 0, 0x0b]),
      ..._body([0x10, 2, 0x0b]),
      ..._body([0x41, 0, 0x41, 0, 0x11, 1, 0, 0x0b]),
      ..._body([0x10, 2, 0x0b]),
      ..._body(extra ?? [0x0b]),
      ..._body([0x0b]),
    ]),
  ]);
}

void main() {
  group('asyncify list', () {
    test('names the callers of the pausing imports, direct and by table', () {
      expect(asyncifyList(_module()), '0,1,2,3');
    });

    test('a function that never reaches an import is not listed', () {
      final listed = asyncifyList(_module()).split(',');
      expect(listed, isNot(contains('4')));
      expect(listed, isNot(contains('5')));
    });

    test('refuses a module whose table can change at run time', () {
      // table.set in function 6 (index 6): i32.const 0; ref.null; table.set 0
      final bytes = _module(extra: [0x41, 0, 0xd0, 0x70, 0x26, 0, 0x0b]);
      expect(() => asyncifyList(bytes), throwsFormatException);
    });

    test('refuses a module that grows its function table', () {
      // ref.null func; i32.const 1; table.grow 0; drop
      final bytes = _module(
        extra: [0xd0, 0x70, 0x41, 1, 0xfc, 15, 0, 0x1a, 0x0b],
      );
      expect(() => asyncifyList(bytes), throwsFormatException);
    });

    test('accepts reads and writes of a table that holds no functions', () {
      // table.get 1, table.set 1, table.grow 1, as wasm-bindgen's externref
      // glue does when the engine keeps its target-features section.
      final bytes = _module(
        extra: [
          0x41, 0, 0x41, 0, 0x25, 1, 0x26, 1, //
          0xd0, 0x6f, 0x41, 1, 0xfc, 15, 1, 0x1a, 0x0b,
        ],
      );
      expect(asyncifyList(bytes), '0,1,2,3');
    });
  });

  group('the wall', () {
    test('passes the list the build computed', () {
      final wasm = _module();
      verifyAsyncifyList(wasm, asyncifyList(wasm));
    });

    test('passes a list with extra functions', () {
      final wasm = _module();
      verifyAsyncifyList(wasm, '${asyncifyList(wasm)},4');
    });

    test('fails when a function reachable from an import is missing', () {
      final wasm = _module();
      expect(
        () => verifyAsyncifyList(wasm, '0,1,3'),
        throwsA(
          isA<StateError>().having((e) => e.message, 'message', contains('2')),
        ),
      );
    });

    test('fails on a list numbered with the imports counted', () {
      // binaryen names a defined function by its index among the defined
      // ones; the same functions numbered across the imports are the wrong ones.
      expect(
        () => verifyAsyncifyList(_module(), '2,3,4,5'),
        throwsA(isA<StateError>()),
      );
    });

    test('fails on a list from another build (stale)', () {
      // A list made for a module without the table caller misses 2 and 3.
      expect(
        () => verifyAsyncifyList(_module(), '0,1'),
        throwsA(isA<StateError>()),
      );
    });
  });
}
