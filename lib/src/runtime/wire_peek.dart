// Minimal wire-format peeks the Router needs for routing decisions.
//
// The Router never DECODES requests or responses (that's the
// protocol codec's job) — it only peeks two facts:
//   - which op a request carries (dispose detection, diagnostics)
//   - the handleId field, if present (pinning)
//
// Wire format (must match binary_codec.rs):
//   Request:  [op_len u8][op utf8][field_count u16 LE][fields...]
//   Response: [status u8][field_count u16 LE][fields...]
//             status 0 → [msg_len u32 LE][msg utf8]
//   Field:    [key_len u8][key utf8][type u8][value]
//   Types:    0=null 1=i32 2=i64 3=f64 4=bool 5=string 6=bytes
//             7=int_list 8=float_list 9=string_list 10=map_list
//
// INTERNAL — used by the Router only.

import 'dart:convert';
import 'dart:typed_data';

/// Reads the op name from request bytes. Returns null on malformed
/// input — the engine owns parse errors, peeks never throw.
String? peekRequestOp(Uint8List request) {
  if (request.isEmpty) return null;
  final opLen = request[0];
  if (request.length < 1 + opLen) return null;
  return utf8.decode(request.sublist(1, 1 + opLen));
}

/// Reads the `handleId` i32 field from request bytes, or null.
int? peekRequestHandleId(Uint8List request) {
  if (request.isEmpty) return null;
  final opLen = request[0];
  final fieldsStart = 1 + opLen + 2;
  if (request.length < fieldsStart) return null;
  final count = ByteData.sublistView(
    request,
    1 + opLen,
    fieldsStart,
  ).getUint16(0, Endian.little);
  return _scanFieldsForHandleId(request, fieldsStart, count);
}

/// Reads the `handleId` i32 field from a SUCCESS response, or null.
/// Error responses (status 0) never carry handles.
int? peekResponseHandleId(Uint8List response) {
  if (response.length < 3 || response[0] != 1) return null;
  final count = ByteData.sublistView(
    response,
    1,
    3,
  ).getUint16(0, Endian.little);
  return _scanFieldsForHandleId(response, 3, count);
}

/// Walks [count] fields starting at [offset]; returns the i32 value
/// of the field keyed `handleId`, or null. Returns null on any
/// malformed structure — peeks never throw.
int? _scanFieldsForHandleId(Uint8List bytes, int offset, int count) {
  var pos = offset;
  for (var i = 0; i < count; i++) {
    if (pos + 1 > bytes.length) return null;
    final keyLen = bytes[pos];
    pos += 1;
    if (pos + keyLen + 1 > bytes.length) return null;
    final key = utf8.decode(bytes.sublist(pos, pos + keyLen));
    pos += keyLen;
    final type = bytes[pos];
    pos += 1;

    final isHandle = key == 'handleId' && type == 1;
    final valueLen = _valueLength(bytes, pos, type);
    if (valueLen == null || pos + valueLen > bytes.length) return null;
    if (isHandle) {
      return ByteData.sublistView(
        bytes,
        pos,
        pos + 4,
      ).getInt32(0, Endian.little);
    }
    pos += valueLen;
  }
  return null;
}

/// Byte length of a field value at [pos], or null if unknown type /
/// truncated input.
int? _valueLength(Uint8List bytes, int pos, int type) {
  switch (type) {
    case 0:
      return 0;
    case 1:
      return 4;
    case 2:
    case 3:
      return 8;
    case 4:
      return 1;
    case 5: // string: [len u32][utf8]
    case 6: // bytes:  [len u32][raw]
      if (pos + 4 > bytes.length) return null;
      final len = ByteData.sublistView(
        bytes,
        pos,
        pos + 4,
      ).getUint32(0, Endian.little);
      return 4 + len;
    case 7: // int_list: [count u32][i64 * count]
      if (pos + 4 > bytes.length) return null;
      final n = ByteData.sublistView(
        bytes,
        pos,
        pos + 4,
      ).getUint32(0, Endian.little);
      return 4 + n * 8;
    case 8: // float_list: [count u32][f64 * count]
      if (pos + 4 > bytes.length) return null;
      final n = ByteData.sublistView(
        bytes,
        pos,
        pos + 4,
      ).getUint32(0, Endian.little);
      return 4 + n * 8;
    default:
      // string_list / map_list / unknown: nothing the Router peeks
      // ever follows one of these — bail out safely.
      return null;
  }
}

/// Builds a cancelled response (status 2) — used by lanes to
/// complete jobs they own after a kill, matching
/// `ResponseWriter::cancelled` on the Rust side.
Uint8List buildCancelledResponse() => Uint8List.fromList(const [2]);

/// Builds an error response carrying [message] — used by lanes for
/// transport-layer failures (worker init failed, pre-copy failed).
/// Layout matches `ResponseWriter::error`:
/// `[0][msg_len u32 LE][msg]`.
Uint8List buildErrorResponse(String message) {
  final msg = utf8.encode(message);
  final out = BytesBuilder(copy: false)
    ..addByte(0)
    ..add(_u32le(msg.length))
    ..add(msg);
  return out.takeBytes();
}

Uint8List _u32le(int v) =>
    Uint8List(4)..buffer.asByteData().setUint32(0, v, Endian.little);
