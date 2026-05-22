import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.probe', () {
    test('valid PDF returns isValid true', () async {
      final info = await pdf.probe(sourceOf(minimalPdf));
      expect(info.isValid, isTrue);
    });

    test('returns page count', () async {
      final info = await pdf.probe(sourceOf(minimalPdf));
      expect(info.pageCount, equals(1));
    });

    test('returns version', () async {
      final info = await pdf.probe(sourceOf(minimalPdf));
      expect(info.version, isNotNull);
      expect(info.version, contains('.'));
    });

    test('isEncrypted false for unencrypted', () async {
      final info = await pdf.probe(sourceOf(minimalPdf));
      expect(info.isEncrypted, isFalse);
    });

    test('garbage bytes return isValid false', () async {
      final info = await pdf.probe(sourceOf(garbageBytes));
      expect(info.isValid, isFalse);
    });

    test('empty bytes return isValid false', () async {
      final info = await pdf.probe(sourceOf(emptyBytes));
      expect(info.isValid, isFalse);
    });

    test('broken PDF with valid header detected as encrypted or invalid', () async {
      final info = await pdf.probe(sourceOf(brokenPdf));
      // brokenPdf starts with %PDF but has garbage after —
      // pdf_oxide either fails to parse (isValid=false) or detects
      // the header and assumes encryption (isValid=true, isEncrypted=true)
      if (info.isValid) {
        // If it thinks it's valid, it must think it's encrypted
        // (can't parse the structure = assumes password needed)
        expect(info.isEncrypted, isTrue);
      } else {
        expect(info.isValid, isFalse);
      }
    });
  });
}
