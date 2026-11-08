// Hybrid isolate asset server whose executor_worker.js can be switched off.
//
// Same job as asset_server.dart, plus a control channel: the test sends
// 'fail' and the server answers every request for executor_worker.js with 404;
// it sends 'ok' and the server serves it again. Lets a test fail a worker
// boot, then let the next one succeed.
//
//   final channel = spawnHybridUri('/test/harness/flaky_asset_server.dart',
//       stayAlive: true);
//   final events = StreamQueue(channel.stream);
//   final port = (await events.next as double).toInt();
//   channel.sink.add('fail');

// io-exempt: hybrid-isolate harness; runs an HttpServer so chrome tests can
// fetch web_assets/.
import 'dart:io';

import 'package:shelf/shelf.dart';
import 'package:shelf/shelf_io.dart' as io;
import 'package:shelf_static/shelf_static.dart';
import 'package:stream_channel/stream_channel.dart';

const _headers = {'Access-Control-Allow-Origin': '*'};

Future<void> hybridMain(StreamChannel<Object?> channel) async {
  var failing = false;
  final server = await HttpServer.bind('localhost', 0);
  final files = createStaticHandler('.');
  io.serveRequests(server, (Request request) async {
    if (request.method == 'OPTIONS') {
      return Response.ok(null, headers: _headers);
    }
    if (failing && request.url.path.endsWith('executor_worker.js')) {
      return Response.notFound('switched off', headers: _headers);
    }
    return (await files(request)).change(headers: _headers);
  });
  channel.sink.add(server.port);
  await channel.stream
      .listen((message) {
        failing = message == 'fail';
        channel.sink.add('switched');
      })
      .asFuture<void>()
      .then<void>((_) => server.close());
}
