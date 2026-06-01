// SharedBridge — contract: every op encoded correctly.
// Mirrors lib/src/transport/shared_bridge.dart.
//
// Verifies that SharedBridge methods produce the correct binary-encoded
// requests. These are UNIT tests against the codec layer — they don't
// need a running engine.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/transport/protocol/binary_codec.dart';
import 'package:pdf_manipulator/src/transport/protocol/codec.dart';
import 'package:pdf_manipulator/src/transport/protocol/op.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
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

    test('editorSaveOp encodes save mode', () {
      final req = editorSaveOp(
          handleId: 1,
          options: const PdfSaveOptions.fullRewrite());
      expect(req.args['saveMode'], 0);
      expect(req.args['compress'], true);
      expect(req.args['garbageCollect'], true);
    });

    test('editorSaveOp encodes incremental', () {
      final req = editorSaveOp(
          handleId: 1,
          options: const PdfSaveOptions.incremental());
      expect(req.args['saveMode'], 1);
      expect(req.args['compress'], false);
      expect(req.args['garbageCollect'], false);
    });

    test('editorSaveOp encodes encryption', () {
      final req = editorSaveOp(
          handleId: 1,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.config(ownerPassword: 'ow'),
          ));
      expect(req.args['encryptMode'], 2);
      expect(req.args['encryptOwnerPw'], 'ow');
    });

    test('editorSaveOp encodes remove encryption', () {
      final req = editorSaveOp(
          handleId: 1,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.remove(),
          ));
      expect(req.args['encryptMode'], 1);
    });

    test('editorMutateOp encodes editOp', () {
      final req = editorMutateOp(handleId: 5, editOp: 'setTitle', extra: {'title': 'x'});
      expect(req.args['handleId'], 5);
      expect(req.args['editOp'], 'setTitle');
      expect(req.args['title'], 'x');
    });

    test('encodeRequest produces valid binary', () {
      final bytes = encodeRequest('open', {'password': 'secret'});
      expect(bytes.length, greaterThan(0));
      // First byte is op name length
      expect(bytes[0], 4);
      expect(String.fromCharCodes(bytes.sublist(1, 5)), 'open');
    });

    test('decodeResponse handles empty response as error', () {
      final map = decodeResponse(Uint8List(0));
      expect(map['error'], 'empty response');
    });
  });
}
