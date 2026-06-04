// NativeTransport — implements PdfTransport for native platforms.
//
// Multi-source/multi-sink: creates N SourceServers + M SinkServers
// per operation. Passes all ports + lengths to the coordinator.

import 'dart:async';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/transport/pdf_transport.dart';
import 'package:pdf_manipulator/src/transport/native/coordinator.dart';
import 'package:pdf_manipulator/src/transport/native/source_server.dart';
import 'package:pdf_manipulator/src/transport/native/sink_server.dart';

/// Native FFI transport — routes ops through a coordinator isolate.
class NativeTransport implements PdfTransport {
  @override
  PdfIoMode? get ioMode => _ready ? PdfIoMode.native : null;

  @override
  Future<PdfIoMode> ensureInitialized() async {
    await _ensureWorker();
    return PdfIoMode.native;
  }

  SendPort? _workerPort;
  Isolate? _workerIsolate;
  ReceivePort? _responsePort;
  bool _ready = false;
  bool _disposed = false;
  Completer<void>? _initCompleter;
  final _pending = <int, Completer<Uint8List>>{};
  final _pendingResourceIds = <int, Completer<Map<int, int>>>{};
  final _pendingStreams = <int, StreamController<Uint8List>>{};
  int _nextId = 0;

  final _heldSourceServers = <int, SourceServer>{};

  Future<void> _ensureWorker() async {
    if (_ready) return;
    if (_disposed) throw StateError('NativeTransport disposed');
    if (_initCompleter != null) {
      await _initCompleter!.future;
      return;
    }

    _initCompleter = Completer<void>();

    final initPort = ReceivePort();
    _workerIsolate = await Isolate.spawn(
      coordinatorEntryPoint,
      initPort.sendPort,
      debugName: 'PdfBridgeCoordinator',
    );
    _workerPort = await initPort.first as SendPort;

    _responsePort = ReceivePort();
    _responsePort!.listen(_onResponse);
    _workerPort!.send(_responsePort!.sendPort);

    _ready = true;
    _initCompleter!.complete();
  }

  void _onResponse(dynamic message) {
    if (message is! List || message.length < 3) return;
    final id = message[0] as int;
    final tag = message[1] as String;
    final payload = message[2];

    switch (tag) {
      case 'result':
        _pending.remove(id)?.complete(payload as Uint8List);
      case 'resourceIds':
        _pendingResourceIds.remove(id)?.complete(
          (payload as Map<int, int>?) ?? const {},
        );
      case 'error':
        _pending.remove(id)?.completeError(
          StateError(payload is String ? payload : 'Bridge error'),
        );
        _pendingResourceIds.remove(id)?.complete(const {});
      case 'item':
        _pendingStreams[id]?.add(payload as Uint8List);
      case 'done':
        unawaited(_pendingStreams.remove(id)?.close());
      case 'streamError':
        final c = _pendingStreams.remove(id);
        c?.addError(StateError(payload as String? ?? 'Stream error'));
        unawaited(c?.close());
    }
  }

  @override
  Future<({Uint8List bytes, Map<int, int> resourceIds})> execute(
    Uint8List request, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
    Set<int> keepSources = const {},
  }) async {
    if (_disposed) throw StateError('NativeTransport disposed');
    await _ensureWorker();
    final id = _nextId++;
    final completer = Completer<Uint8List>();
    _pending[id] = completer;

    final resourceIdCompleter = keepSources.isNotEmpty
        ? Completer<Map<int, int>>() : null;
    if (resourceIdCompleter != null) {
      _pendingResourceIds[id] = resourceIdCompleter;
    }

    // Create source servers
    final sourceServers = <int, SourceServer>{};
    final sourcePorts = <SendPort?>[];
    final sourceLengths = <int>[];
    for (var i = 0; i < sources.length; i++) {
      final server = SourceServer(sources[i]);
      sourceServers[i] = server;
      sourcePorts.add(server.start());
      sourceLengths.add(sources[i].length);
    }

    // Create sink servers
    final sinkServers = <int, SinkServer>{};
    final sinkPorts = <SendPort?>[];
    for (var i = 0; i < sinks.length; i++) {
      final server = SinkServer(sinks[i]);
      sinkServers[i] = server;
      sinkPorts.add(server.start());
    }

    _workerPort!.send([
      id,
      'exec',
      request,
      sourcePorts,
      sourceLengths,
      sinkPorts,
      keepSources,
    ]);

    try {
      final bytes = await completer.future;

      var resourceIds = const <int, int>{};
      if (resourceIdCompleter != null) {
        resourceIds = await resourceIdCompleter.future;
        for (final entry in resourceIds.entries) {
          final srcIdx = entry.key;
          final resId = entry.value;
          if (sourceServers.containsKey(srcIdx)) {
            _heldSourceServers[resId] = sourceServers[srcIdx]!;
            sourceServers.remove(srcIdx);
          }
        }
      }

      return (bytes: bytes, resourceIds: resourceIds);
    } finally {
      // Stop servers NOT kept for a handle
      for (final server in sourceServers.values) {
        server.stop();
      }
      for (final server in sinkServers.values) {
        server.stop();
      }
    }
  }

  @override
  Future<void> releaseSource(int resourceId) async {
    _heldSourceServers.remove(resourceId)?.stop();

    if (_disposed || !_ready) return;
    await _ensureWorker();
    final id = _nextId++;
    final completer = Completer<Uint8List>();
    _pending[id] = completer;

    _workerPort!.send([id, 'releaseSource', resourceId]);

    await completer.future;
  }

  @override
  Stream<Uint8List> executeStream(
    Uint8List request, {
    List<DataSource> sources = const [],
  }) async* {
    if (_disposed) throw StateError('NativeTransport disposed');
    await _ensureWorker();
    final id = _nextId++;
    final controller = StreamController<Uint8List>();
    _pendingStreams[id] = controller;

    final sourceServers = <SourceServer>[];
    final sourcePorts = <SendPort?>[];
    final sourceLengths = <int>[];
    for (final source in sources) {
      final server = SourceServer(source);
      sourceServers.add(server);
      sourcePorts.add(server.start());
      sourceLengths.add(source.length);
    }

    _workerPort!.send([
      id,
      'execStream',
      request,
      sourcePorts,
      sourceLengths,
      <SendPort?>[],
      <int>{},
    ]);

    try {
      await for (final chunk in controller.stream) {
        yield chunk;
      }
    } finally {
      for (final server in sourceServers) {
        server.stop();
      }
      _pendingStreams.remove(id);
      unawaited(controller.close());
    }
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;

    for (final s in _heldSourceServers.values) {
      s.stop();
    }
    _heldSourceServers.clear();

    // Send shutdown to coordinator — cancels buffers (wakes blocked
    // Rust threads), calls bridgeShutdown (joins threads), then closes
    // NativeCallables. Must complete before killing the isolate.
    if (_workerPort != null && _ready) {
      final id = _nextId++;
      final completer = Completer<Uint8List>();
      _pending[id] = completer;
      _workerPort!.send([id, 'shutdown']);
      try {
        await completer.future.timeout(Duration(seconds: 5));
      } catch (_) {
        // Timeout — kill anyway below
      }
    }

    _pending.clear();
    _pendingResourceIds.clear();
    _pendingStreams.clear();

    _responsePort?.close();
    _workerIsolate?.kill(priority: Isolate.immediate);
    _workerIsolate = null;
  }
}
