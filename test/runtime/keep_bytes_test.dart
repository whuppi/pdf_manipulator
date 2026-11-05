// The keep line has one home, in Rust; Dart mirrors it.
//
// io-exempt: reads the engine's heap.rs from disk to compare a constant.

@TestOn('vm')
library;

import 'dart:io';

import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:test/test.dart';

/// `KEEP_BYTES` from heap.rs: a product of integer literals.
int _rustKeepBytes() {
  final source = File('vendor/pdf_oxide/src/host/heap.rs').readAsStringSync();
  final match = RegExp(
    r'pub const KEEP_BYTES: usize = ([0-9_]+(?:\s*\*\s*[0-9_]+)*);',
  ).firstMatch(source);
  if (match == null) throw StateError('KEEP_BYTES not found in heap.rs');
  return match
      .group(1)!
      .split('*')
      .map((factor) => int.parse(factor.trim().replaceAll('_', '')))
      .reduce((a, b) => a * b);
}

void main() {
  test('kKeepBytes is the engine\'s KEEP_BYTES', () {
    expect(kKeepBytes, _rustKeepBytes());
  });
}
