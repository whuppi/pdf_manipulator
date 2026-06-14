import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart';
import 'package:test/test.dart';

void main() {
  group('encodeRequest + decodeResponse round-trip', () {
    test('empty fields', () {
      final bytes = encodeRequest('open', {});
      expect(bytes[0], 4); // op length
      expect(String.fromCharCodes(bytes.sublist(1, 5)), 'open');
      // field count = 0
      expect(bytes[5], 0);
      expect(bytes[6], 0);
    });

    test('null fields are excluded', () {
      final bytes = encodeRequest('open', {'password': null, 'page': 3});
      // Only 'page' should be encoded (null excluded)
      final r = _parseRequest(bytes);
      expect(r.fields.length, 1);
      expect(r.fields['page'], 3);
    });

    test('int32 value', () {
      final bytes = encodeRequest('test', {'count': 42});
      final r = _parseRequest(bytes);
      expect(r.op, 'test');
      expect(r.fields['count'], 42);
    });

    test('negative int32', () {
      final bytes = encodeRequest('test', {'offset': -100});
      final r = _parseRequest(bytes);
      expect(r.fields['offset'], -100);
    });

    test('int64 value (large)', () {
      final big = 0x100000000; // > 32-bit
      final bytes = encodeRequest('test', {'size': big});
      final r = _parseRequest(bytes);
      expect(r.fields['size'], big);
    });

    test('double value', () {
      final bytes = encodeRequest('test', {'opacity': 0.75});
      final r = _parseRequest(bytes);
      expect(r.fields['opacity'], closeTo(0.75, 1e-10));
    });

    test('bool values', () {
      final bytes = encodeRequest('test', {'compress': true, 'gc': false});
      final r = _parseRequest(bytes);
      expect(r.fields['compress'], true);
      expect(r.fields['gc'], false);
    });

    test('string value', () {
      final bytes = encodeRequest('test', {'title': 'Hello World'});
      final r = _parseRequest(bytes);
      expect(r.fields['title'], 'Hello World');
    });

    test('empty string', () {
      final bytes = encodeRequest('test', {'text': ''});
      final r = _parseRequest(bytes);
      expect(r.fields['text'], '');
    });

    test('unicode string', () {
      final bytes = encodeRequest('test', {'name': '日本語テスト'});
      final r = _parseRequest(bytes);
      expect(r.fields['name'], '日本語テスト');
    });

    test('bytes value', () {
      final data = Uint8List.fromList([0xDE, 0xAD, 0xBE, 0xEF]);
      final bytes = encodeRequest('test', {'data': data});
      final r = _parseRequest(bytes);
      expect(r.fields['data'], data);
    });

    test('large bytes value', () {
      final data = Uint8List(100000);
      for (var i = 0; i < data.length; i++) {
        data[i] = i & 0xFF;
      }
      final bytes = encodeRequest('test', {'payload': data});
      final r = _parseRequest(bytes);
      expect(r.fields['payload'], data);
    });

    test('int list', () {
      final bytes = encodeRequest('test', {
        'pages': [0, 5, 10],
      });
      final r = _parseRequest(bytes);
      expect(r.fields['pages'], [0, 5, 10]);
    });

    test('empty int list', () {
      final bytes = encodeRequest('test', {'pages': <int>[]});
      final r = _parseRequest(bytes);
      expect(r.fields['pages'], <int>[]);
    });

    test('float list', () {
      final bytes = encodeRequest('test', {
        'coords': [1.5, 2.5, 3.5],
      });
      final r = _parseRequest(bytes);
      final coords = r.fields['coords'] as List<double>;
      expect(coords.length, 3);
      expect(coords[0], closeTo(1.5, 1e-10));
      expect(coords[2], closeTo(3.5, 1e-10));
    });

    test('string list', () {
      final bytes = encodeRequest('test', {
        'tags': ['pdf', 'test', 'unicode: ñ'],
      });
      final r = _parseRequest(bytes);
      expect(r.fields['tags'], ['pdf', 'test', 'unicode: ñ']);
    });

    test('multiple fields of different types', () {
      final data = Uint8List.fromList([1, 2, 3]);
      final bytes = encodeRequest('editorMutate', {
        'handleId': 7,
        'editOp': 'watermark',
        'text': 'DRAFT',
        'opacity': 0.3,
        'compress': true,
        'pages': [0, 1, 2],
        'data': data,
      });
      final r = _parseRequest(bytes);
      expect(r.op, 'editorMutate');
      expect(r.fields['handleId'], 7);
      expect(r.fields['editOp'], 'watermark');
      expect(r.fields['text'], 'DRAFT');
      expect(r.fields['opacity'], closeTo(0.3, 1e-10));
      expect(r.fields['compress'], true);
      expect(r.fields['pages'], [0, 1, 2]);
      expect(r.fields['data'], data);
    });
  });

  group('decodeResponse', () {
    test('error response', () {
      final buf = BytesBuilder();
      buf.addByte(0); // status = error
      final msg = 'something broke';
      final msgBytes = utf8.encode(msg);
      _addU32(buf, msgBytes.length);
      buf.add(msgBytes);
      final result = decodeResponse(buf.takeBytes());
      expect(result['error'], 'something broke');
    });

    test('success response with fields', () {
      final response = _buildResponse({
        'pageCount': 10,
        'version': '2.0',
        'isEncrypted': false,
      });
      final result = decodeResponse(response);
      expect(result['pageCount'], 10);
      expect(result['version'], '2.0');
      expect(result['isEncrypted'], false);
    });

    test('empty response bytes', () {
      final result = decodeResponse(Uint8List(0));
      expect(result.containsKey('error'), true);
    });

    test('response with bytes field', () {
      final payload = Uint8List.fromList([0xFF, 0xFE, 0xFD]);
      final response = _buildResponse({'data': payload});
      final result = decodeResponse(response);
      expect(result['data'], payload);
    });

    test('response with nested list of maps', () {
      final response = _buildResponse({
        'hits': [
          {'page': 0, 'text': 'hello'},
          {'page': 1, 'text': 'world'},
        ],
      });
      final result = decodeResponse(response);
      final hits = result['hits'] as List<Map<String, Object?>>;
      expect(hits.length, 2);
      expect(hits[0]['page'], 0);
      expect(hits[0]['text'], 'hello');
      expect(hits[1]['page'], 1);
      expect(hits[1]['text'], 'world');
    });
  });

  group('full round-trip encode→decode', () {
    test('request fields survive encode→decode via response', () {
      final original = {
        'handleId': 42,
        'editOp': 'watermark',
        'text': 'CONFIDENTIAL',
        'opacity': 0.5,
        'compress': true,
        'pages': [0, 1, 2, 3],
      };
      final response = _buildResponse(original);
      final decoded = decodeResponse(response);
      expect(decoded['handleId'], 42);
      expect(decoded['editOp'], 'watermark');
      expect(decoded['text'], 'CONFIDENTIAL');
      expect(decoded['opacity'], closeTo(0.5, 1e-10));
      expect(decoded['compress'], true);
      expect(decoded['pages'], [0, 1, 2, 3]);
    });
  });
}

// Helpers to verify request encoding by parsing it back.
({String op, Map<String, Object?> fields}) _parseRequest(Uint8List bytes) {
  var pos = 0;
  final opLen = bytes[pos++];
  final op = utf8.decode(bytes.sublist(pos, pos + opLen));
  pos += opLen;
  final fieldCount = _readU16(bytes, pos);
  pos += 2;
  final fields = <String, Object?>{};
  for (var i = 0; i < fieldCount; i++) {
    final keyLen = bytes[pos++];
    final key = utf8.decode(bytes.sublist(pos, pos + keyLen));
    pos += keyLen;
    final result = _readValueAt(bytes, pos);
    fields[key] = result.value;
    pos = result.nextPos;
  }
  return (op: op, fields: fields);
}

({Object? value, int nextPos}) _readValueAt(Uint8List bytes, int pos) {
  final bd = ByteData.sublistView(bytes);
  final type = bytes[pos++];
  switch (type) {
    case 1: // i32
      return (value: bd.getInt32(pos, Endian.little), nextPos: pos + 4);
    case 2: // i64
      return (value: bd.getInt64(pos, Endian.little), nextPos: pos + 8);
    case 3: // f64
      return (value: bd.getFloat64(pos, Endian.little), nextPos: pos + 8);
    case 4: // bool
      return (value: bytes[pos] != 0, nextPos: pos + 1);
    case 5: // string
      final len = bd.getUint32(pos, Endian.little);
      pos += 4;
      final s = utf8.decode(bytes.sublist(pos, pos + len));
      return (value: s, nextPos: pos + len);
    case 6: // bytes
      final len = bd.getUint32(pos, Endian.little);
      pos += 4;
      return (
        value: Uint8List.sublistView(bytes, pos, pos + len),
        nextPos: pos + len,
      );
    case 7: // int list
      final count = bd.getUint32(pos, Endian.little);
      pos += 4;
      final list = <int>[];
      for (var i = 0; i < count; i++) {
        list.add(bd.getInt32(pos, Endian.little));
        pos += 4;
      }
      return (value: list, nextPos: pos);
    case 8: // float list
      final count = bd.getUint32(pos, Endian.little);
      pos += 4;
      final list = <double>[];
      for (var i = 0; i < count; i++) {
        list.add(bd.getFloat64(pos, Endian.little));
        pos += 8;
      }
      return (value: list, nextPos: pos);
    case 9: // string list
      final count = bd.getUint32(pos, Endian.little);
      pos += 4;
      final list = <String>[];
      for (var i = 0; i < count; i++) {
        final sLen = bd.getUint32(pos, Endian.little);
        pos += 4;
        list.add(utf8.decode(bytes.sublist(pos, pos + sLen)));
        pos += sLen;
      }
      return (value: list, nextPos: pos);
    default:
      return (value: null, nextPos: pos);
  }
}

int _readU16(Uint8List bytes, int pos) =>
    ByteData.sublistView(bytes).getUint16(pos, Endian.little);

// Build a valid response from a field map (for testing decodeResponse).
Uint8List _buildResponse(Map<String, Object?> fields) {
  final buf = BytesBuilder();
  buf.addByte(1); // status = ok
  final entries = fields.entries.where((e) => e.value != null).toList();
  _addU16(buf, entries.length);
  for (final e in entries) {
    _addField(buf, e.key, e.value!);
  }
  return buf.takeBytes();
}

void _addField(BytesBuilder buf, String key, Object value) {
  final keyBytes = utf8.encode(key);
  buf.addByte(keyBytes.length);
  buf.add(keyBytes);
  _addValue(buf, value);
}

void _addValue(BytesBuilder buf, Object value) {
  switch (value) {
    case final int v:
      if (v.abs() <= 0x7FFFFFFF) {
        buf.addByte(1);
        final bd = ByteData(4)..setInt32(0, v, Endian.little);
        buf.add(bd.buffer.asUint8List());
      } else {
        buf.addByte(2);
        final bd = ByteData(8)..setInt64(0, v, Endian.little);
        buf.add(bd.buffer.asUint8List());
      }
    case final double v:
      buf.addByte(3);
      final bd = ByteData(8)..setFloat64(0, v, Endian.little);
      buf.add(bd.buffer.asUint8List());
    case final bool v:
      buf.addByte(4);
      buf.addByte(v ? 1 : 0);
    case final String v:
      buf.addByte(5);
      final sb = utf8.encode(v);
      _addU32(buf, sb.length);
      buf.add(sb);
    case final Uint8List v:
      buf.addByte(6);
      _addU32(buf, v.length);
      buf.add(v);
    case final List<int> v:
      buf.addByte(7);
      _addU32(buf, v.length);
      for (final n in v) {
        final bd = ByteData(4)..setInt32(0, n, Endian.little);
        buf.add(bd.buffer.asUint8List());
      }
    case final List<Map<String, Object?>> v:
      buf.addByte(10);
      _addU32(buf, v.length);
      for (final m in v) {
        final nested = _buildNestedMap(m);
        _addU32(buf, nested.length);
        buf.add(nested);
      }
  }
}

Uint8List _buildNestedMap(Map<String, Object?> m) {
  final buf = BytesBuilder();
  final entries = m.entries.where((e) => e.value != null).toList();
  _addU16(buf, entries.length);
  for (final e in entries) {
    _addField(buf, e.key, e.value!);
  }
  return buf.takeBytes();
}

void _addU16(BytesBuilder buf, int v) {
  buf.addByte(v & 0xFF);
  buf.addByte((v >> 8) & 0xFF);
}

void _addU32(BytesBuilder buf, int v) {
  buf.addByte(v & 0xFF);
  buf.addByte((v >> 8) & 0xFF);
  buf.addByte((v >> 16) & 0xFF);
  buf.addByte((v >> 24) & 0xFF);
}
