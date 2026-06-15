// FileSource — positioned reads over a file on disk. Native only, so this
// test needs `dart:io` (test-guards exempts it for that reason).

@TestOn('vm')
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:pdf_manipulator/io.dart';
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

  group('FileSource', () {
    test('length is read from the file at construction', () {
      expect(FileSource(file).length, 100);
    });

    test('readAt returns the requested window', () async {
      expect(
        await FileSource(file).readAt(10, 5),
        equals([10, 11, 12, 13, 14]),
      );
    });

    test('readAt near EOF returns only the available bytes', () async {
      expect(await FileSource(file).readAt(96, 50), equals([96, 97, 98, 99]));
    });

    test('readAt past EOF returns empty', () async {
      expect(await FileSource(file).readAt(100, 10), isEmpty);
      expect(await FileSource(file).readAt(200, 10), isEmpty);
    });

    test(
      'concurrent reads stay independent (open-per-read, no shared position)',
      () async {
        // The reason readAt opens a fresh handle each call: parallel reads must
        // not race on a shared file position. Fire several at once.
        final source = FileSource(file);
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
      },
    );
  });
}
