// wire_peek — the Router's minimal wire reads.
//
// Round-trips real encoder output through the peeks, plus the
// malformed-input contract: a peek never throws, it returns null and
// lets the engine produce the real parse error.

@TestOn('vm')
library;

import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:test/test.dart';

/// Hand-rolls a SUCCESS response carrying one i32 `handleId` field —
/// the shape `ResponseWriter` emits for handle-creating ops.
Uint8List _successWithHandle(int handleId) {
  final key = 'handleId'.codeUnits;
  final out = BytesBuilder()
    ..addByte(1) // status: success
    ..add([1, 0]) // field count u16 LE
    ..addByte(key.length)
    ..add(key)
    ..addByte(1) // type: i32
    ..add(
      Uint8List(4)..buffer.asByteData().setInt32(0, handleId, Endian.little),
    );
  return out.takeBytes();
}

void main() {
  group('peekRequestOp', () {
    test('round-trips the encoder output', () {
      final req = bin.encodeRequest('editorMergeFrom', {'sourceLength': 9});
      expect(peekRequestOp(req), 'editorMergeFrom');
    });

    test('returns null on empty and truncated input', () {
      expect(peekRequestOp(Uint8List(0)), isNull);
      expect(peekRequestOp(Uint8List.fromList([200])), isNull);
    });
  });

  group('peekRequestHandleId', () {
    test('finds handleId among other fields', () {
      final req = bin.encodeRequest('extract', {
        'format': 'text',
        'handleId': 7,
        'page': 3,
      });
      expect(peekRequestHandleId(req), 7);
    });

    test('returns null when absent', () {
      final req = bin.encodeRequest('open', {'sourceLength': 100});
      expect(peekRequestHandleId(req), isNull);
    });

    test('returns null on malformed input instead of throwing', () {
      expect(peekRequestHandleId(Uint8List(0)), isNull);
      expect(
        peekRequestHandleId(Uint8List.fromList([4, 111, 112, 101])),
        isNull,
      );
    });
  });

  group('peekResponseHandleId', () {
    test('reads the handle from a success response', () {
      expect(peekResponseHandleId(_successWithHandle(42)), 42);
    });

    test('error and cancelled responses never carry handles', () {
      expect(peekResponseHandleId(buildErrorResponse('boom')), isNull);
      expect(peekResponseHandleId(buildCancelledResponse()), isNull);
    });
  });

  group('response builders', () {
    test('cancelled response is the single status byte 2', () {
      // Must match ResponseWriter::cancelled — the typed-cancel wire
      // contract the whole dispose/cancel path rests on.
      expect(buildCancelledResponse(), [2]);
    });

    test('error response matches ResponseWriter::error_kind layout', () {
      final bytes = buildErrorResponse('boom');
      expect(bytes[0], 0); // status = error
      expect(bytes[1], 0); // kind = Engine
      expect(ByteData.sublistView(bytes, 2, 4).getUint16(0, Endian.little), 1);
      expect(bytes[4], 7); // key length of "message"
      expect(String.fromCharCodes(bytes.sublist(5, 12)), 'message');
      expect(bytes[12], 5); // string type
      expect(
        ByteData.sublistView(bytes, 13, 17).getUint32(0, Endian.little),
        4,
      );
      expect(String.fromCharCodes(bytes.sublist(17)), 'boom');
    });
  });
}
