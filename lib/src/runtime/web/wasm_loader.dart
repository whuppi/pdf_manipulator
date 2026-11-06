// The wasm manifest and the main thread's loader of a wasm module.
//
// Part of lease.dart. Every module the package ships is cut into parts listed
// in pdf_oxide_manifest.json (see lib/src/hook/wasm_parts.dart); nothing
// fetches a whole `.wasm`. [_compileParts] is the main-thread twin of
// compileParts in executor_worker.js, which the worker runs when the host's
// compile failed: both fetch every part at once, join them in order into one
// stream the browser compiles while it arrives, and refuse a part that is not
// the size and SHA-256 the manifest lists, with a message naming the part.
// The web battery pins both against the same manifest.

part of 'lease.dart';

/// `pdf_oxide_manifest.json`: the modules the app shipped, by name.
class _WasmManifest {
  _WasmManifest._(this.modules);

  /// Reads a manifest; a [PdfUnsupported] says what is wrong with [text].
  factory _WasmManifest.parse(String text) {
    final Object? json;
    try {
      json = jsonDecode(text);
    } on FormatException {
      throw const PdfUnsupported('not JSON');
    }
    if (json is! Map<String, dynamic>) {
      throw const PdfUnsupported('not an object of modules');
    }
    return _WasmManifest._({
      for (final MapEntry(:key, :value) in json.entries)
        key: _WasmModuleEntry.parse(key, value),
    });
  }

  final Map<String, _WasmModuleEntry> modules;
}

/// One module of the manifest: its parts, in the order they join.
class _WasmModuleEntry {
  _WasmModuleEntry._(this.json, this.parts);

  factory _WasmModuleEntry.parse(String name, Object? value) {
    if (value is! Map<String, dynamic> || value['parts'] is! List) {
      throw PdfUnsupported('$name has no parts');
    }
    final parts = <_WasmPart>[];
    for (final part in value['parts'] as List) {
      if (part is! Map<String, dynamic> ||
          part['file'] is! String ||
          part['bytes'] is! int ||
          part['sha256'] is! String) {
        throw PdfUnsupported('$name lists a part without file, bytes, sha256');
      }
      parts.add(
        _WasmPart(
          part['file'] as String,
          part['bytes'] as int,
          part['sha256'] as String,
        ),
      );
    }
    if (parts.isEmpty) throw PdfUnsupported('$name has no parts');
    return _WasmModuleEntry._(value, parts);
  }

  /// The entry as the manifest has it: what the worker gets in `init`.
  final Map<String, dynamic> json;
  final List<_WasmPart> parts;
}

class _WasmPart {
  _WasmPart(this.file, this.bytes, this.sha256);

  final String file;
  final int bytes;
  final String sha256;
}

@JS('Response')
extension type _WasmResponse._(JSObject _) implements JSObject {
  external factory _WasmResponse(JSObject body, JSObject init);
}

/// Compiles [module] from its parts at [baseUrl], as the stream described at
/// the top of this file. Rejects with a [PdfIoFailed] naming the part at fault.
Future<JSObject> _compileParts(String baseUrl, _WasmModuleEntry module) async {
  final abort = web.AbortController();
  final fetches = [
    for (final part in module.parts)
      web.window
          .fetch(
            '$baseUrl${part.file}'.toJS,
            web.RequestInit(signal: abort.signal),
          )
          .toDart
        ..ignore(),
  ];
  Object? failure;
  final source = JSObject()
    ..['start'] = ((web.ReadableStreamDefaultController controller) {
      return (() async {
        try {
          for (final (i, part) in module.parts.indexed) {
            await _pumpPart(await fetches[i], part, controller);
          }
          controller.close();
        } catch (error) {
          failure = error;
          abort.abort();
          controller.error('$error'.toJS);
        }
      })().toJS;
    }).toJS;
  final response = _WasmResponse(
    web.ReadableStream(source),
    {
          'headers': {'content-type': 'application/wasm'},
        }.jsify()!
        as JSObject,
  );
  try {
    return await _wasmCompileStreaming(response).toDart;
  } catch (error) {
    throw failure ?? error;
  }
}

/// Enqueues the bytes of [response] as they arrive, and refuses them at the
/// end unless they are [part]: its size, then its SHA-256.
Future<void> _pumpPart(
  web.Response response,
  _WasmPart part,
  web.ReadableStreamDefaultController controller,
) async {
  if (!response.ok || response.body == null) {
    throw PdfIoFailed('${part.file}: HTTP ${response.status}');
  }
  final reader = response.body!.getReader() as web.ReadableStreamDefaultReader;
  final bytes = BytesBuilder(copy: false);
  var count = 0;
  while (true) {
    final next = await reader.read().toDart;
    if (next.done) break;
    final chunk = next.value! as JSUint8Array;
    final data = chunk.toDart;
    count += data.length;
    if (count > part.bytes) {
      throw PdfIoFailed(
        '${part.file}: more than the ${part.bytes} bytes the manifest lists',
      );
    }
    bytes.add(data);
    controller.enqueue(chunk);
  }
  if (count != part.bytes) {
    throw PdfIoFailed(
      '${part.file}: $count bytes, the manifest lists ${part.bytes}',
    );
  }
  final hex = await _sha256Hex(bytes.takeBytes());
  if (hex != part.sha256) {
    throw PdfIoFailed(
      '${part.file}: sha256 $hex is not the ${part.sha256} the manifest '
      'lists',
    );
  }
}

/// The SHA-256 of [bytes] as lowercase hex: the browser's own digest where the
/// page has `crypto.subtle` (a secure context), else package:crypto's.
Future<String> _sha256Hex(Uint8List bytes) async {
  final List<int> digest;
  if ((web.window.crypto as JSObject)['subtle'] != null) {
    final buffer = await web.window.crypto.subtle
        .digest('SHA-256'.toJS, bytes.toJS)
        .toDart;
    digest = (buffer! as JSArrayBuffer).toDart.asUint8List();
  } else {
    digest = crypto.sha256.convert(bytes).bytes;
  }
  return [
    for (final byte in digest) byte.toRadixString(16).padLeft(2, '0'),
  ].join();
}
