import 'dart:convert';
import 'dart:typed_data';

// Wire types — same values the Rust parser expects.
const _tNull = 0;
const _tI32 = 1;
const _tI64 = 2;
const _tF64 = 3;
const _tBool = 4;
const _tString = 5;
const _tBytes = 6;
const _tIntList = 7;
const _tF64List = 8;
const _tStringList = 9;
const _tMapList = 10;

Uint8List encodeRequest(String op, Map<String, Object?> fields) {
  final buf = BytesBuilder(copy: false);
  final opBytes = utf8.encode(op);
  buf.addByte(opBytes.length);
  buf.add(opBytes);
  final entries = fields.entries.where((e) => e.value != null).toList();
  _writeU16(buf, entries.length);
  for (final e in entries) {
    _writeField(buf, e.key, e.value!);
  }
  return buf.takeBytes();
}

Map<String, Object?> decodeResponse(Uint8List bytes) {
  if (bytes.isEmpty) return {'error': 'empty response'};
  final r = _Reader(bytes);
  final status = r.u8();
  if (status == 0) {
    final msgLen = r.u32();
    final msg = utf8.decode(r.bytes(msgLen));
    return {'error': msg};
  }
  final fieldCount = r.u16();
  final map = <String, Object?>{};
  for (var i = 0; i < fieldCount; i++) {
    final keyLen = r.u8();
    final key = utf8.decode(r.bytes(keyLen));
    map[key] = _readValue(r);
  }
  return map;
}

void _writeField(BytesBuilder buf, String key, Object value) {
  final keyBytes = utf8.encode(key);
  buf.addByte(keyBytes.length);
  buf.add(keyBytes);
  _writeValue(buf, value);
}

void _writeValue(BytesBuilder buf, Object value) {
  switch (value) {
    case int v:
      if (v.abs() <= 0x7FFFFFFF) {
        buf.addByte(_tI32);
        _writeI32(buf, v);
      } else {
        buf.addByte(_tI64);
        _writeI64(buf, v);
      }
    case double v:
      buf.addByte(_tF64);
      _writeF64(buf, v);
    case bool v:
      buf.addByte(_tBool);
      buf.addByte(v ? 1 : 0);
    case String v:
      buf.addByte(_tString);
      final strBytes = utf8.encode(v);
      _writeU32(buf, strBytes.length);
      buf.add(strBytes);
    case Uint8List v:
      buf.addByte(_tBytes);
      _writeU32(buf, v.length);
      buf.add(v);
    case List<int> v:
      buf.addByte(_tIntList);
      _writeU32(buf, v.length);
      for (final n in v) {
        _writeI32(buf, n);
      }
    case List<double> v:
      buf.addByte(_tF64List);
      _writeU32(buf, v.length);
      for (final n in v) {
        _writeF64(buf, n);
      }
    case List<String> v:
      buf.addByte(_tStringList);
      _writeU32(buf, v.length);
      for (final s in v) {
        final sb = utf8.encode(s);
        _writeU32(buf, sb.length);
        buf.add(sb);
      }
    case List<Uint8List> v:
      buf.addByte(_tMapList);
      _writeU32(buf, v.length);
      for (final item in v) {
        _writeU32(buf, item.length);
        buf.add(item);
      }
    case List<Map<String, Object?>> v:
      buf.addByte(_tMapList);
      _writeU32(buf, v.length);
      for (final m in v) {
        final nested = _encodeNestedMap(m);
        _writeU32(buf, nested.length);
        buf.add(nested);
      }
    case Map<String, Object?> v:
      final nested = _encodeNestedMap(v);
      buf.addByte(_tBytes);
      _writeU32(buf, nested.length);
      buf.add(nested);
    default:
      if (value is List) {
        if (value.every((e) => e is int)) {
          buf.addByte(_tIntList);
          _writeU32(buf, value.length);
          for (final n in value) {
            _writeI32(buf, n as int);
          }
        } else if (value.every((e) => e is Uint8List)) {
          buf.addByte(_tMapList);
          _writeU32(buf, value.length);
          for (final item in value) {
            final b = item as Uint8List;
            _writeU32(buf, b.length);
            buf.add(b);
          }
        } else if (value.every((e) => e is Map)) {
          buf.addByte(_tMapList);
          _writeU32(buf, value.length);
          for (final item in value) {
            final nested = _encodeNestedMap(item as Map<String, Object?>);
            _writeU32(buf, nested.length);
            buf.add(nested);
          }
        }
      }
  }
}

Uint8List _encodeNestedMap(Map<String, Object?> m) {
  final buf = BytesBuilder(copy: false);
  final entries = m.entries.where((e) => e.value != null).toList();
  _writeU16(buf, entries.length);
  for (final e in entries) {
    _writeField(buf, e.key, e.value!);
  }
  return buf.takeBytes();
}

Object? _readValue(_Reader r) {
  final type = r.u8();
  return switch (type) {
    _tNull => null,
    _tI32 => r.i32(),
    _tI64 => r.i64(),
    _tF64 => r.f64(),
    _tBool => r.u8() != 0,
    _tString => _readString(r),
    _tBytes => _readBytes(r),
    _tIntList => _readIntList(r),
    _tF64List => _readF64List(r),
    _tStringList => _readStringList(r),
    _tMapList => _readMapList(r),
    _ => null,
  };
}

String _readString(_Reader r) {
  final len = r.u32();
  return utf8.decode(r.bytes(len));
}

Uint8List _readBytes(_Reader r) {
  final len = r.u32();
  return r.bytesAsUint8List(len);
}

List<int> _readIntList(_Reader r) {
  final count = r.u32();
  return List.generate(count, (_) => r.i32());
}

List<double> _readF64List(_Reader r) {
  final count = r.u32();
  return List.generate(count, (_) => r.f64());
}

List<String> _readStringList(_Reader r) {
  final count = r.u32();
  return List.generate(count, (_) => _readString(r));
}

List<Map<String, Object?>> _readMapList(_Reader r) {
  final count = r.u32();
  return List.generate(count, (_) {
    final len = r.u32();
    final mapBytes = r.bytesAsUint8List(len);
    final mr = _Reader(mapBytes);
    final fieldCount = mr.u16();
    final map = <String, Object?>{};
    for (var i = 0; i < fieldCount; i++) {
      final keyLen = mr.u8();
      final key = utf8.decode(mr.bytes(keyLen));
      map[key] = _readValue(mr);
    }
    return map;
  });
}

void _writeU16(BytesBuilder buf, int v) {
  buf.addByte(v & 0xFF);
  buf.addByte((v >> 8) & 0xFF);
}

void _writeU32(BytesBuilder buf, int v) {
  buf.addByte(v & 0xFF);
  buf.addByte((v >> 8) & 0xFF);
  buf.addByte((v >> 16) & 0xFF);
  buf.addByte((v >> 24) & 0xFF);
}

void _writeI32(BytesBuilder buf, int v) {
  final bd = ByteData(4)..setInt32(0, v, Endian.little);
  buf.add(bd.buffer.asUint8List());
}

void _writeI64(BytesBuilder buf, int v) {
  final bd = ByteData(8)..setInt64(0, v, Endian.little);
  buf.add(bd.buffer.asUint8List());
}

void _writeF64(BytesBuilder buf, double v) {
  final bd = ByteData(8)..setFloat64(0, v, Endian.little);
  buf.add(bd.buffer.asUint8List());
}

class _Reader {
  _Reader(this._data) : _bd = ByteData.sublistView(_data);
  final Uint8List _data;
  final ByteData _bd;
  int _pos = 0;

  int u8() => _data[_pos++];

  int u16() {
    final v = _bd.getUint16(_pos, Endian.little);
    _pos += 2;
    return v;
  }

  int u32() {
    final v = _bd.getUint32(_pos, Endian.little);
    _pos += 4;
    return v;
  }

  int i32() {
    final v = _bd.getInt32(_pos, Endian.little);
    _pos += 4;
    return v;
  }

  int i64() {
    final v = _bd.getInt64(_pos, Endian.little);
    _pos += 8;
    return v;
  }

  double f64() {
    final v = _bd.getFloat64(_pos, Endian.little);
    _pos += 8;
    return v;
  }

  List<int> bytes(int count) {
    final slice = _data.sublist(_pos, _pos + count);
    _pos += count;
    return slice;
  }

  Uint8List bytesAsUint8List(int count) {
    final slice = Uint8List.sublistView(_data, _pos, _pos + count);
    _pos += count;
    return slice;
  }
}
