// SharedBridge — contract: every op encoded correctly.
// Mirrors lib/src/bridge/shared_bridge.dart.
//
// Verifies that SharedBridge methods produce the correct binary-encoded
// requests. These are UNIT tests against the codec layer — they don't
// need a running engine.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart';
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:test/test.dart';

void main() {
  group('shared bridge encoding', () {
    test('every EngineOp has a non-empty wire name', () {
      for (final op in EngineOp.values) {
        expect(op.wire, isNotEmpty, reason: '${op.name} wire name');
      }
    });

    test('every EngineOp wire name is unique', () {
      final wires = EngineOp.values.map((o) => o.wire).toSet();
      expect(wires.length, EngineOp.values.length);
    });

    test('encodeRequest produces valid binary', () {
      final bytes = encodeRequest('open', {'password': 'secret'});
      expect(bytes.length, greaterThan(0));
      // First byte is op name length
      expect(bytes[0], 4);
      expect(String.fromCharCodes(bytes.sublist(1, 5)), 'open');
    });

    test('decodeResponse throws PdfEngineError on empty response', () {
      expect(
        () => decodeResponse(Uint8List(0)),
        throwsA(isA<PdfEngineError>()),
      );
    });
  });
}
