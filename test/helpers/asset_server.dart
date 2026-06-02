// Hybrid isolate asset server for web tests.
//
// dart test -p chrome serves test code but NOT the package root.
// This server makes web_assets/ (coordinator.js, worker.js, *.wasm)
// fetchable from the browser test via http://localhost:<port>/web_assets/...
//
// Usage:
//   final channel = spawnHybridUri('/test/helpers/asset_server.dart', stayAlive: true);
//   final port = (await channel.stream.first as double).toInt();

import 'dart:io';

import 'package:shelf/shelf.dart';
import 'package:shelf/shelf_io.dart' as io;
import 'package:shelf_static/shelf_static.dart';
import 'package:stream_channel/stream_channel.dart';

Middleware _cors() => createMiddleware(
      requestHandler: (req) =>
          req.method == 'OPTIONS' ? Response.ok(null, headers: _headers) : null,
      responseHandler: (res) => res.change(headers: _headers),
    );

const _headers = {'Access-Control-Allow-Origin': '*'};

Future<void> hybridMain(StreamChannel<Object?> channel) async {
  final server = await HttpServer.bind('localhost', 0);
  io.serveRequests(server,
      const Pipeline().addMiddleware(_cors()).addHandler(createStaticHandler('.')));
  channel.sink.add(server.port);
  await channel.stream.listen(null).asFuture<void>().then<void>((_) => server.close());
}
