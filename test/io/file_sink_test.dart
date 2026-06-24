// FileSink — streams written chunks to a file on disk.
// io-exempt: native-only; this writes a real on-disk file.

@TestOn('vm')
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:pdf_manipulator/io.dart';
import 'package:test/test.dart';

void main() {
  late Directory dir;
  late File file;

  setUp(() {
    dir = Directory.systemTemp.createTempSync('pdfm_filesink_');
    file = File('${dir.path}/out.bin');
  });
  tearDown(() => dir.deleteSync(recursive: true));

  group('FileSink', () {
    test('writes chunks in order and flushes on close', () async {
      final sink = await FileSink.create(file);
      await sink.write(Uint8List.fromList([1, 2, 3]));
      await sink.write(Uint8List.fromList([4, 5]));
      await sink.close();
      expect(file.readAsBytesSync(), equals([1, 2, 3, 4, 5]));
    });

    test('create truncates existing content', () async {
      file.writeAsBytesSync(Uint8List.fromList([9, 9, 9, 9, 9]));
      final sink = await FileSink.create(file);
      await sink.write(Uint8List.fromList([1, 2]));
      await sink.close();
      expect(file.readAsBytesSync(), equals([1, 2]));
    });

    test(
      'close is idempotent — a second call is a no-op, not a throw',
      () async {
        final sink = await FileSink.create(file);
        await sink.write(Uint8List.fromList([1, 2, 3]));
        await sink.close();
        // Regression: the old close() threw "file closed" on the second call.
        await sink.close();
        expect(file.readAsBytesSync(), equals([1, 2, 3]));
      },
    );
  });
}
