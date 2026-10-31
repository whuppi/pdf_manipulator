// FileSource — positioned reads over a file on disk.
// io-exempt: native-only; this reads a real on-disk file.

@TestOn('vm')
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:pdf_manipulator/io.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  late Directory dir;
  late File file;
  final bytes = Uint8List.fromList(List.generate(100, (i) => i));

  setUp(() {
    dir = Directory.systemTemp.createTempSync('pdfm_filesource_');
    file = File('${dir.path}/data.bin')..writeAsBytesSync(bytes);
  });
  tearDown(() => dir.deleteSync(recursive: true));

  Future<FileSource> open() => FileSource.open(file);

  group('FileSource', () {
    test('length is read from the file when it opens', () async {
      final source = await open();
      expect(source.length, 100);
      await source.close();
    });

    test('readAt returns the requested window', () async {
      final source = await open();
      expect(await source.readAt(10, 5), equals([10, 11, 12, 13, 14]));
      await source.close();
    });

    test('readAt near EOF returns only the available bytes', () async {
      final source = await open();
      expect(await source.readAt(96, 50), equals([96, 97, 98, 99]));
      await source.close();
    });

    test('readAt past EOF returns empty', () async {
      final source = await open();
      expect(await source.readAt(100, 10), isEmpty);
      expect(await source.readAt(200, 10), isEmpty);
      await source.close();
    });

    test('concurrent reads on the one handle stay independent', () async {
      // One handle, one position: reads queue, so parallel callers never
      // move each other's position.
      final source = await open();
      final results = await Future.wait([
        source.readAt(0, 4),
        source.readAt(20, 4),
        source.readAt(40, 4),
        source.readAt(60, 4),
      ]);
      expect(results[0], equals([0, 1, 2, 3]));
      expect(results[1], equals([20, 21, 22, 23]));
      expect(results[2], equals([40, 41, 42, 43]));
      expect(results[3], equals([60, 61, 62, 63]));
      await source.close();
    });

    test('close is idempotent and a read after it is PdfDisposed', () async {
      final source = await open();
      await source.close();
      await source.close();
      expect(() => source.readAt(0, 1), throwsA(isA<PdfDisposed>()));
    });

    test('close waits for the reads in flight', () async {
      final source = await open();
      final pending = source.readAt(5, 3);
      await source.close();
      expect(await pending, equals([5, 6, 7]));
    });
  });
}
