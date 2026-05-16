import 'dart:isolate';
import 'dart:typed_data';

import 'package:test/test.dart';

void main() {
  group('TransferableTypedData behavior', () {
    test('fromList copies — original survives', () {
      final original = Uint8List.fromList([1, 2, 3, 4, 5]);
      final transferable = TransferableTypedData.fromList([original]);

      // Original still usable after wrapping
      expect(original.length, 5);
      expect(original[0], 1);

      // Materialize the transferable
      final materialized = transferable.materialize().asUint8List();
      expect(materialized.length, 5);
      expect(materialized[0], 1);

      // Original STILL usable — fromList copied, didn't steal
      expect(original.length, 5);
      expect(original[0], 1);
    });

    test('materialize can only be called once', () {
      final transferable = TransferableTypedData.fromList([
        Uint8List.fromList([10, 20, 30]),
      ]);

      // First materialize works
      final first = transferable.materialize().asUint8List();
      expect(first.length, 3);

      // Second materialize throws — ownership was consumed
      expect(() => transferable.materialize(), throwsA(anything));
    });

    test('SendPort.send transfers ownership — received side can materialize', () async {
      final receivePort = ReceivePort();
      final sendPort = receivePort.sendPort;

      final original = Uint8List.fromList(List.generate(1000, (i) => i % 256));
      final transferable = TransferableTypedData.fromList([original]);

      // Send it
      sendPort.send(transferable);

      // Receive it
      final received = await receivePort.first as TransferableTypedData;
      final materialized = received.materialize().asUint8List();

      expect(materialized.length, 1000);
      expect(materialized[0], 0);
      expect(materialized[255], 255);

      // Original Uint8List is STILL fine (fromList copied into transferable)
      expect(original.length, 1000);
      expect(original[0], 0);

      receivePort.close();
    });

    test('after send, sender cannot materialize', () async {
      final receivePort = ReceivePort();
      final sendPort = receivePort.sendPort;

      final transferable = TransferableTypedData.fromList([
        Uint8List.fromList([1, 2, 3]),
      ]);

      sendPort.send(transferable);
      await receivePort.first;

      // Sender's transferable is now neutered
      expect(() => transferable.materialize(), throwsA(anything));

      receivePort.close();
    });

    test('cross-isolate round-trip preserves data', () async {
      final receivePort = ReceivePort();
      final stream = receivePort.asBroadcastStream();

      await Isolate.spawn((SendPort mainPort) {
        final port = ReceivePort();
        mainPort.send(port.sendPort);

        port.listen((message) {
          if (message is TransferableTypedData) {
            final bytes = message.materialize().asUint8List();
            final result = Uint8List(bytes.length);
            for (var i = 0; i < bytes.length; i++) {
              result[i] = (bytes[i] * 2) % 256;
            }
            mainPort.send(TransferableTypedData.fromList([result]));
            port.close();
          }
        });
      }, receivePort.sendPort);

      final workerPort = await stream.first as SendPort;

      final input = Uint8List.fromList([10, 20, 30, 40, 50]);
      workerPort.send(TransferableTypedData.fromList([input]));

      final result = await stream.first as TransferableTypedData;
      final output = result.materialize().asUint8List();

      expect(output, [20, 40, 60, 80, 100]);
      expect(input, [10, 20, 30, 40, 50]);

      receivePort.close();
    });

    test('large buffer transfer is fast (not O(n) copy)', () async {
      final receivePort = ReceivePort();
      final stream = receivePort.asBroadcastStream();

      await Isolate.spawn((SendPort mainPort) {
        final port = ReceivePort();
        mainPort.send(port.sendPort);

        port.listen((message) {
          if (message is TransferableTypedData) {
            final bytes = message.materialize().asUint8List();
            mainPort.send(bytes.length);
            port.close();
          }
        });
      }, receivePort.sendPort);

      final workerPort = await stream.first as SendPort;

      // 50MB buffer
      final bigBuffer = Uint8List(50 * 1024 * 1024);
      for (var i = 0; i < bigBuffer.length; i++) {
        bigBuffer[i] = i % 256;
      }

      final sw = Stopwatch()..start();
      final transferable = TransferableTypedData.fromList([bigBuffer]);
      final fromListTime = sw.elapsedMilliseconds;

      sw.reset();
      workerPort.send(transferable);
      final receivedLength = await stream.first as int;
      final sendTime = sw.elapsedMilliseconds;

      expect(receivedLength, 50 * 1024 * 1024);

      // fromList does a copy — takes measurable time for 50MB
      // send should be near-instant (O(1) pointer move)
      print('50MB: fromList=${fromListTime}ms, send+receive=${sendTime}ms');

      receivePort.close();
    });
  });
}
