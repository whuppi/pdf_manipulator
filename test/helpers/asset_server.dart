// Asset server for web tests — serves the package root with CORS + COOP/COEP.
//
// Started via spawnHybridUri('/test/helpers/asset_server.dart').
// Serves web_assets/ (coordinator.js, wasm_worker.js, WASM binary).
// COOP/COEP headers enable SharedArrayBuffer for Atomics mode.
//

import 'dart:io';

import 'package:shelf/shelf.dart';
import 'package:shelf/shelf_io.dart' as io;
import 'package:shelf_static/shelf_static.dart';
import 'package:stream_channel/stream_channel.dart';

const _headers = {
  'Access-Control-Allow-Origin': '*',
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
};

Middleware _cors() {
  return createMiddleware(
    requestHandler: (request) {
      if (request.method == 'OPTIONS') return Response.ok(null, headers: _headers);
      return null;
    },
    responseHandler: (response) => response.change(headers: _headers),
  );
}

Future<void> hybridMain(StreamChannel<Object?> channel) async {
  final server = await HttpServer.bind('localhost', 0);

  final handler = const Pipeline()
      .addMiddleware(_cors())
      .addHandler(createStaticHandler('.'));
  io.serveRequests(server, handler);

  channel.sink.add(server.port);
  await channel.stream
      .listen(null)
      .asFuture<void>()
      .then<void>((_) => server.close());
}
