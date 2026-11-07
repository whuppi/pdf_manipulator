// PDF date strings (ISO 32000-1 §7.9.4): parse to UTC, format back, and
// never throw on a string that is not a date.

import 'package:pdf_manipulator/src/bridge/protocol/pdf_date.dart';
import 'package:test/test.dart';

void main() {
  group('parsePdfDate', () {
    test('a full date with Z', () {
      expect(
        parsePdfDate('D:20240115123045Z'),
        DateTime.utc(2024, 1, 15, 12, 30, 45),
      );
    });

    test('the year alone', () {
      expect(parsePdfDate('D:2024'), DateTime.utc(2024));
    });

    test('each trailing part is optional and defaults low', () {
      expect(parsePdfDate('D:202403'), DateTime.utc(2024, 3));
      expect(parsePdfDate('D:20240315'), DateTime.utc(2024, 3, 15));
      expect(parsePdfDate('D:2024031509'), DateTime.utc(2024, 3, 15, 9));
      expect(parsePdfDate('D:202403150930'), DateTime.utc(2024, 3, 15, 9, 30));
    });

    test('a positive offset is subtracted to reach UTC', () {
      expect(
        parsePdfDate("D:20240115123045+05'30'"),
        DateTime.utc(2024, 1, 15, 7, 0, 45),
      );
    });

    test('a negative offset is added to reach UTC', () {
      expect(
        parsePdfDate("D:20240115123045-08'00'"),
        DateTime.utc(2024, 1, 15, 20, 30, 45),
      );
    });

    test('an offset crossing midnight changes the day', () {
      expect(
        parsePdfDate("D:20240101003000+02'00'"),
        DateTime.utc(2023, 12, 31, 22, 30),
      );
    });

    test('the offset without apostrophes, and with hours only', () {
      expect(
        parsePdfDate('D:20240115123045+0530'),
        DateTime.utc(2024, 1, 15, 7, 0, 45),
      );
      expect(
        parsePdfDate("D:20240115123045-08'"),
        DateTime.utc(2024, 1, 15, 20, 30, 45),
      );
      expect(
        parsePdfDate('D:20240115123045+05'),
        DateTime.utc(2024, 1, 15, 7, 30, 45),
      );
    });

    test('no offset reads as UTC', () {
      expect(
        parsePdfDate('D:20240115123045'),
        DateTime.utc(2024, 1, 15, 12, 30, 45),
      );
    });

    test('the D: prefix is optional', () {
      expect(parsePdfDate('20240115'), DateTime.utc(2024, 1, 15));
    });

    test('garbage gives null, never a throw', () {
      for (final bad in [
        '',
        'D:',
        'not a date',
        'D:20',
        'D:20241',
        'D:2024011',
        'D:20241301',
        'D:20240001',
        'D:20240230',
        'D:20240115250000',
        'D:20240115126000',
        'D:20240115123060',
        'D:20240115123045+99',
        'D:20240115123045X',
        'D:20240115123045Zjunk',
      ]) {
        expect(parsePdfDate(bad), isNull, reason: bad);
      }
      expect(parsePdfDate(null), isNull);
    });
  });

  group('formatPdfDate', () {
    test('writes UTC with Z', () {
      expect(
        formatPdfDate(DateTime.utc(2024, 1, 15, 12, 30, 45)),
        'D:20240115123045Z',
      );
    });

    test('writes the UTC moment of a time given with an offset', () {
      final withOffset = DateTime.parse('2024-01-15T12:30:45+05:30');
      expect(formatPdfDate(withOffset), 'D:20240115070045Z');
    });

    test('pads every part', () {
      expect(
        formatPdfDate(DateTime.utc(900, 2, 3, 4, 5, 6)),
        'D:09000203040506Z',
      );
    });

    test('round-trips through parse', () {
      final at = DateTime.utc(2031, 12, 31, 23, 59, 59);
      expect(parsePdfDate(formatPdfDate(at)), at);
    });
  });
}
