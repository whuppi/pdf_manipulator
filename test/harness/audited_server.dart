// A running audited_asset_server.dart, from the test's side: its port and the
// requests it has seen.

import 'dart:async';

import 'package:stream_channel/stream_channel.dart';
import 'package:test/test.dart';

/// A server that hides some paths and lists every request it gets.
class AuditedServer {
  AuditedServer._(this.port, this._events, this.requests);

  final int port;
  final StreamIterator<Object?> _events;
  final List<String> requests;

  static Future<AuditedServer> start(List<String> hide) async {
    final StreamChannel<Object?> channel = spawnHybridUri(
      '/test/harness/audited_asset_server.dart',
      message: hide,
      stayAlive: true,
    );
    final events = StreamIterator(channel.stream);
    await events.moveNext();
    final audit = AuditedServer._(
      (events.current as double).toInt(),
      events,
      [],
    );
    unawaited(audit._collect());
    return audit;
  }

  Future<void> _collect() async {
    while (await _events.moveNext()) {
      requests.add(_events.current! as String);
    }
  }

  String get workerUrl =>
      'http://localhost:$port/web_assets/executor_worker.js';

  /// The requests whose path ends in `file`, as `METHOD path`.
  List<String> asked(String file) => [
    for (final r in requests)
      if (r.endsWith(file)) r,
  ];

  /// Lets requests the server has sent but this side has not read yet arrive.
  Future<void> settle() =>
      Future<void>.delayed(const Duration(milliseconds: 300));
}
