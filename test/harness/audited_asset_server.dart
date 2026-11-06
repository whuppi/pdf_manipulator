// Hybrid isolate asset server that reports what the browser asks for and can
// hide files.
//
// Same job as asset_server.dart. The spawn message is a list of paths (as
// requested, e.g. `web_assets/pdf_oxide_asyncify_bg.wasm`) the server answers
// with 404. The channel carries the port first, then one string per request,
// `METHOD path`, in the order the server saw them.
//
//   final channel = spawnHybridUri('/test/harness/audited_asset_server.dart',
//       message: ['web_assets/pdf_oxide_asyncify_bg.wasm'], stayAlive: true);
//   final events = StreamQueue(channel.stream);
//   final port = (await events.next as double).toInt();

// io-exempt: hybrid-isolate harness; runs an HttpServer so chrome tests can
// fetch web_assets/.
import 'dart:io';

import 'package:shelf/shelf.dart';
import 'package:shelf/shelf_io.dart' as io;
import 'package:shelf_static/shelf_static.dart';
import 'package:stream_channel/stream_channel.dart';

const _headers = {'Access-Control-Allow-Origin': '*'};

Future<void> hybridMain(StreamChannel<Object?> channel, Object? message) async {
  final hidden = {for (final path in (message as List?) ?? const []) '$path'};
  final server = await HttpServer.bind('localhost', 0);
  final files = createStaticHandler('.');
  io.serveRequests(server, (Request request) async {
    if (request.method == 'OPTIONS') {
      return Response.ok(null, headers: _headers);
    }
    channel.sink.add('${request.method} ${request.url.path}');
    if (hidden.contains(request.url.path)) {
      return Response.notFound('hidden', headers: _headers);
    }
    return (await files(request)).change(headers: _headers);
  });
  channel.sink.add(server.port);
  await channel.stream
      .listen(null)
      .asFuture<void>()
      .then<void>((_) => server.close());
}
