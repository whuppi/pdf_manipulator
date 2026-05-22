// Hybrid VM isolate that serves WASM assets to browser tests.
// Called via spawnHybridUri() from web e2e tests.
//
// Starts a shelf_static server rooted at the package directory,
// sends back the port number, stays alive until the channel closes.

import 'package:shelf/shelf.dart' as shelf;
import 'package:shelf/shelf_io.dart' as shelf_io;
import 'package:shelf_static/shelf_static.dart';
import 'package:stream_channel/stream_channel.dart';

void hybridMain(StreamChannel<Object?> channel) async {
  final handler = createStaticHandler(
    '.',
    defaultDocument: 'index.html',
    serveFilesOutsidePath: true,
  );

  final server = await shelf_io.serve(
    const shelf.Pipeline()
        .addMiddleware(shelf.createMiddleware(
          responseHandler: (response) => response.change(headers: {
            'Access-Control-Allow-Origin': '*',
            'Cross-Origin-Opener-Policy': 'same-origin',
            'Cross-Origin-Embedder-Policy': 'require-corp',
          }),
        ))
        .addHandler(handler),
    'localhost',
    0,
  );

  channel.sink.add(server.port);

  await channel.stream.drain<void>();
  await server.close();
}
