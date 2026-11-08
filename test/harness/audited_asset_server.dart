// Hybrid isolate asset server that reports what the browser asks for, can hide
// files, and can serve a web host's faults.
//
// Same job as asset_server.dart. The spawn message is a map:
//
//   hide             paths (as requested, e.g.
//                    `web_assets/pdf_oxide_asyncify_bg.0.wasm`) the server
//                    answers with 404;
//   spa              true: a single-page-app host, which answers every path
//                    that is hidden or has no file with 200 and an HTML page;
//   manifestWithout  module names the served pdf_oxide_manifest.json leaves
//                    out, as the manifest of an app that did not ship them;
//   corrupt          paths whose served body has one byte changed;
//   truncate         paths whose served body lacks its last bytes.
//
// The channel carries the port first, then one string per request,
// `METHOD path`, in the order the server saw them.
//
//   final channel = spawnHybridUri('/test/harness/audited_asset_server.dart',
//       message: {'hide': ['web_assets/pdf_oxide_asyncify_bg.0.wasm']},
//       stayAlive: true);
//   final events = StreamQueue(channel.stream);
//   final port = (await events.next as double).toInt();

// io-exempt: hybrid-isolate harness; runs an HttpServer so chrome tests can
// fetch web_assets/.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:shelf/shelf.dart';
import 'package:shelf/shelf_io.dart' as io;
import 'package:shelf_static/shelf_static.dart';
import 'package:stream_channel/stream_channel.dart';

const _headers = {'Access-Control-Allow-Origin': '*'};
const _manifest = 'web_assets/pdf_oxide_manifest.json';

Future<void> hybridMain(StreamChannel<Object?> channel, Object? message) async {
  final options = (message as Map?) ?? const {};
  Set<String> paths(String key) => {
    for (final path in (options[key] as List?) ?? const []) '$path',
  };
  final hidden = paths('hide');
  final corrupt = paths('corrupt');
  final truncate = paths('truncate');
  final without = paths('manifestWithout');
  final spa = options['spa'] == true;
  final server = await HttpServer.bind('localhost', 0);
  final files = createStaticHandler('.');
  Response page() => Response.ok(
    '<!doctype html><title>app</title>',
    headers: {..._headers, 'content-type': 'text/html'},
  );
  io.serveRequests(server, (Request request) async {
    if (request.method == 'OPTIONS') {
      return Response.ok(null, headers: _headers);
    }
    final path = request.url.path;
    channel.sink.add('${request.method} $path');
    if (hidden.contains(path)) {
      return spa ? page() : Response.notFound('hidden', headers: _headers);
    }
    final response = (await files(request)).change(headers: _headers);
    if (response.statusCode == 404 && spa) return page();
    if (response.statusCode != 200 || request.method != 'GET') return response;
    if (path == _manifest && without.isNotEmpty) {
      final json = jsonDecode(await response.readAsString()) as Map;
      json.removeWhere((module, _) => without.contains(module));
      return Response.ok(
        jsonEncode(json),
        headers: {..._headers, 'content-type': 'application/json'},
      );
    }
    if (corrupt.contains(path) || truncate.contains(path)) {
      // One copy into a byte buffer: a part is 16 MiB, and a list of ints
      // built byte by byte takes seconds on a slow runner.
      final builder = BytesBuilder(copy: false);
      await response.read().forEach(builder.add);
      var bytes = builder.takeBytes();
      if (corrupt.contains(path)) bytes[bytes.length ~/ 2] ^= 0xff;
      if (truncate.contains(path)) {
        bytes = Uint8List.sublistView(bytes, 0, bytes.length - 16);
      }
      return Response.ok(
        bytes,
        headers: {..._headers, 'content-type': 'application/wasm'},
      );
    }
    return response;
  });
  channel.sink.add(server.port);
  await channel.stream
      .listen(null)
      .asFuture<void>()
      .then<void>((_) => server.close());
}
