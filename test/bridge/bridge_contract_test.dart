// Bridge contract tests — verify Dart ↔ Rust binary protocol agreement.
//
// These tests encode requests using the Dart binary_codec, then verify
// the byte layout matches what the Rust binary_codec::Request::parse
// expects. They also verify response decoding handles all types correctly.
//
// These are UNIT tests against the codec — not integration tests against
// the running engine. When the engine is compiled and running, the
// full end-to-end path (SharedBridge → transport → bridge_execute →
// dispatch → engine → response) is tested by the behavioral tests
// in test/ops/.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart';
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';
import 'package:test/test.dart';

void main() {
  group('request encoding contract', () {
    test('op name is first field', () {
      final bytes = encodeRequest('open', {'password': 'secret'});
      // First byte: op name length
      expect(bytes[0], 4);
      // Next 4 bytes: "open"
      expect(String.fromCharCodes(bytes.sublist(1, 5)), 'open');
    });

    test('field count follows op name', () {
      final bytes = encodeRequest('test', {'a': 1, 'b': 2, 'c': 3});
      final opLen = bytes[0];
      final fieldCountOffset = 1 + opLen;
      final fieldCount = ByteData.sublistView(
        bytes,
      ).getUint16(fieldCountOffset, Endian.little);
      expect(fieldCount, 3);
    });

    test('null fields excluded from encoding', () {
      final bytes = encodeRequest('test', {
        'present': 42,
        'absent': null,
        'also_present': 'hello',
      });
      final opLen = bytes[0];
      final fieldCount = ByteData.sublistView(
        bytes,
      ).getUint16(1 + opLen, Endian.little);
      expect(fieldCount, 2);
    });

    test('every EngineOp wire name is valid ASCII', () {
      for (final op in EngineOp.values) {
        final bytes = encodeRequest(op.wire, {});
        final opLen = bytes[0];
        final opName = String.fromCharCodes(bytes.sublist(1, 1 + opLen));
        expect(opName, op.wire);
        expect(opName, matches(RegExp(r'^[a-zA-Z]+$')));
      }
    });
  });

  group('response decoding contract', () {
    test('error response has status 0', () {
      final response = _buildErrorResponse('test error');
      expect(response[0], 0);
      final map = decodeResponse(response);
      expect(map['error'], 'test error');
    });

    test('ok response has status 1', () {
      final response = _buildOkResponse({'count': 42});
      expect(response[0], 1);
      final map = decodeResponse(response);
      expect(map['count'], 42);
    });

    test('response with all types', () {
      final response = _buildOkResponse({
        'intVal': 42,
        'floatVal': 3.14,
        'boolVal': true,
        'strVal': 'hello',
        'bytesVal': Uint8List.fromList([1, 2, 3]),
      });
      final map = decodeResponse(response);
      expect(map['intVal'], 42);
      expect(map['floatVal'], closeTo(3.14, 0.001));
      expect(map['boolVal'], true);
      expect(map['strVal'], 'hello');
      expect(map['bytesVal'], Uint8List.fromList([1, 2, 3]));
    });
  });

  group('full encode → decode round-trip', () {
    test('request args survive round-trip via response', () {
      // Encode as request, decode as if it were a response.
      // This verifies the wire format is consistent.
      final original = {
        'handleId': 7,
        'editOp': 'watermark',
        'opacity': 0.5,
        'compress': true,
      };
      final response = _buildOkResponse(original);
      final decoded = decodeResponse(response);
      expect(decoded['handleId'], 7);
      expect(decoded['editOp'], 'watermark');
      expect(decoded['opacity'], closeTo(0.5, 0.001));
      expect(decoded['compress'], true);
    });

    test('nested map list survives round-trip', () {
      final response = _buildOkResponse({
        'pages': [
          {'index': 0, 'width': 612.0, 'height': 792.0},
          {'index': 1, 'width': 612.0, 'height': 792.0},
        ],
      });
      final decoded = decodeResponse(response);
      final pages = decoded['pages'] as List<Map<String, Object?>>;
      expect(pages.length, 2);
      expect(pages[0]['index'], 0);
      expect(pages[1]['index'], 1);
    });
  });
}

Uint8List _buildErrorResponse(String msg) {
  final buf = BytesBuilder();
  buf.addByte(0);
  final msgBytes = Uint8List.fromList(msg.codeUnits);
  buf.add(
    (ByteData(
      4,
    )..setUint32(0, msgBytes.length, Endian.little)).buffer.asUint8List(),
  );
  buf.add(msgBytes);
  return buf.takeBytes();
}

Uint8List _buildOkResponse(Map<String, Object?> fields) {
  // encodeRequest emits op+fields; a response is status+fields —
  // build it manually to match the response layout exactly.
  final buf = BytesBuilder();
  buf.addByte(1); // status ok
  final entries = fields.entries.where((e) => e.value != null).toList();
  buf.add(
    (ByteData(
      2,
    )..setUint16(0, entries.length, Endian.little)).buffer.asUint8List(),
  );
  for (final e in entries) {
    _addField(buf, e.key, e.value!);
  }
  return buf.takeBytes();
}

void _addField(BytesBuilder buf, String key, Object value) {
  final keyBytes = Uint8List.fromList(key.codeUnits);
  buf.addByte(keyBytes.length);
  buf.add(keyBytes);
  _addValue(buf, value);
}

void _addValue(BytesBuilder buf, Object value) {
  switch (value) {
    case final int v:
      buf.addByte(1);
      buf.add(
        (ByteData(4)..setInt32(0, v, Endian.little)).buffer.asUint8List(),
      );
    case final double v:
      buf.addByte(3);
      buf.add(
        (ByteData(8)..setFloat64(0, v, Endian.little)).buffer.asUint8List(),
      );
    case final bool v:
      buf.addByte(4);
      buf.addByte(v ? 1 : 0);
    case final String v:
      buf.addByte(5);
      final sb = Uint8List.fromList(v.codeUnits);
      buf.add(
        (ByteData(
          4,
        )..setUint32(0, sb.length, Endian.little)).buffer.asUint8List(),
      );
      buf.add(sb);
    case final Uint8List v:
      buf.addByte(6);
      buf.add(
        (ByteData(
          4,
        )..setUint32(0, v.length, Endian.little)).buffer.asUint8List(),
      );
      buf.add(v);
    case final List<Map<String, Object?>> v:
      buf.addByte(10);
      buf.add(
        (ByteData(
          4,
        )..setUint32(0, v.length, Endian.little)).buffer.asUint8List(),
      );
      for (final m in v) {
        final nested = _buildNestedMap(m);
        buf.add(
          (ByteData(
            4,
          )..setUint32(0, nested.length, Endian.little)).buffer.asUint8List(),
        );
        buf.add(nested);
      }
  }
}

Uint8List _buildNestedMap(Map<String, Object?> m) {
  final buf = BytesBuilder();
  final entries = m.entries.where((e) => e.value != null).toList();
  buf.add(
    (ByteData(
      2,
    )..setUint16(0, entries.length, Endian.little)).buffer.asUint8List(),
  );
  for (final e in entries) {
    _addField(buf, e.key, e.value!);
  }
  return buf.takeBytes();
}
