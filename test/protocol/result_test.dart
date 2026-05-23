// Tests for shared result parsers.
// Verifies parsing of mock engine result maps into typed API objects.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/protocol/result.dart';
import 'package:test/test.dart';

void main() {
  group('parseOpenResult', () {
    test('parses a complete open result', () {
      final doc = parseOpenResult({
        'pageCount': 3,
        'version': '1.7',
        'title': 'Test PDF',
        'author': 'DC',
        'isTagged': true,
        'isEncrypted': false,
        'pages': [
          {'index': 0, 'width': 612.0, 'height': 792.0, 'rotation': 0},
          {'index': 1, 'width': 612.0, 'height': 792.0, 'rotation': 90},
          {'index': 2, 'width': 842.0, 'height': 595.0, 'rotation': 0},
        ],
      });
      expect(doc.pageCount, 3);
      expect(doc.version, '1.7');
      expect(doc.title, 'Test PDF');
      expect(doc.author, 'DC');
      expect(doc.isTagged, isTrue);
      expect(doc.isEncrypted, isFalse);
      expect(doc.pages, hasLength(3));
      expect(doc.pages[0].width, 612.0);
      expect(doc.pages[1].rotation, 90);
      expect(doc.pages[2].width, 842.0);
    });

    test('handles missing optional fields', () {
      final doc = parseOpenResult({
        'pageCount': 1,
        'pages': [
          {'index': 0, 'width': 100.0, 'height': 200.0},
        ],
      });
      expect(doc.version, '2.0');
      expect(doc.title, isNull);
      expect(doc.author, isNull);
      expect(doc.isTagged, isFalse);
      expect(doc.isEncrypted, isFalse);
      expect(doc.pages[0].rotation, 0);
    });

    test('handles empty pages list', () {
      final doc = parseOpenResult({'pageCount': 0});
      expect(doc.pages, isEmpty);
    });
  });

  group('parseSearchResults', () {
    test('parses search hits', () {
      final results = parseSearchResults({
        'hits': [
          {'page': 0, 'text': 'hello', 'x': 72.0, 'y': 700.0, 'width': 50.0, 'height': 12.0},
          {'page': 2, 'text': 'world', 'x': 100.0, 'y': 500.0, 'width': 60.0, 'height': 14.0},
        ],
      });
      expect(results, hasLength(2));
      expect(results[0].page, 0);
      expect(results[0].text, 'hello');
      expect(results[0].rect.x, 72.0);
      expect(results[1].page, 2);
      expect(results[1].rect.width, 60.0);
    });

    test('handles empty hits', () {
      expect(parseSearchResults({}), isEmpty);
    });

    test('handles missing text', () {
      final results = parseSearchResults({
        'hits': [
          {'page': 0, 'x': 0.0, 'y': 0.0, 'width': 10.0, 'height': 10.0},
        ],
      });
      expect(results[0].text, '');
    });
  });

  group('parseSignatures', () {
    test('parses signature list', () {
      final sigs = parseSignatures({
        'signatures': [
          {
            'signerName': 'Alice',
            'reason': 'Approval',
            'location': 'NYC',
            'signingTime': '2025-01-15T10:30:00Z',
            'isValid': true,
          },
        ],
      });
      expect(sigs, hasLength(1));
      expect(sigs[0].signerName, 'Alice');
      expect(sigs[0].reason, 'Approval');
      expect(sigs[0].location, 'NYC');
      expect(sigs[0].signingTime, isNotNull);
      expect(sigs[0].isValid, isTrue);
    });

    test('handles null signing time', () {
      final sigs = parseSignatures({
        'signatures': [{'isValid': false}],
      });
      expect(sigs[0].signingTime, isNull);
      expect(sigs[0].signerName, isNull);
    });

    test('handles empty signatures', () {
      expect(parseSignatures({}), isEmpty);
    });
  });

  group('parseValidationResult', () {
    test('parses compliant result', () {
      final r = parseValidationResult({'compliant': true, 'errors': 0, 'warnings': 2});
      expect(r.compliant, isTrue);
      expect(r.errors, 0);
      expect(r.warnings, 2);
    });

    test('defaults to non-compliant on empty map', () {
      final r = parseValidationResult({});
      expect(r.compliant, isFalse);
      expect(r.errors, 0);
      expect(r.warnings, 0);
    });
  });

  group('parseRenderedPage', () {
    test('parses with ByteBuffer data', () {
      final bytes = Uint8List.fromList([1, 2, 3, 4]);
      final page = parseRenderedPage({
        'width': 800,
        'height': 600,
        'data': bytes.buffer,
      });
      expect(page.width, 800);
      expect(page.height, 600);
      expect(page.data, hasLength(4));
    });

    test('parses with Uint8List data', () {
      final bytes = Uint8List.fromList([5, 6, 7]);
      final page = parseRenderedPage({
        'width': 100,
        'height': 200,
        'data': bytes,
      });
      expect(page.data, hasLength(3));
    });

    test('handles missing data', () {
      final page = parseRenderedPage({'width': 10, 'height': 10});
      expect(page.data, isEmpty);
    });

    test('handles missing dimensions', () {
      final page = parseRenderedPage({});
      expect(page.width, 0);
      expect(page.height, 0);
    });
  });

  group('parsePdfImage', () {
    test('parses full image data', () {
      final bytes = Uint8List.fromList([0xFF, 0xD8, 0xFF]);
      final img = parsePdfImage({
        'width': 1920,
        'height': 1080,
        'format': 'jpeg',
        'colorSpace': 'RGB',
        'bitsPerComponent': 8,
        'data': bytes,
      });
      expect(img.width, 1920);
      expect(img.height, 1080);
      expect(img.format, 'jpeg');
      expect(img.colorSpace, 'RGB');
      expect(img.bitsPerComponent, 8);
      expect(img.data, hasLength(3));
    });

    test('handles defaults', () {
      final img = parsePdfImage({});
      expect(img.format, '');
      expect(img.colorSpace, '');
      expect(img.bitsPerComponent, 8);
      expect(img.data, isEmpty);
    });
  });

  group('parseEditorMetadata', () {
    test('parses all metadata fields', () {
      final m = parseEditorMetadata({
        'pageCount': 5,
        'version': '1.4',
        'title': 'My Doc',
        'author': 'DC',
        'subject': 'Testing',
        'keywords': 'pdf test',
      });
      expect(m.pageCount, 5);
      expect(m.version, '1.4');
      expect(m.title, 'My Doc');
      expect(m.author, 'DC');
      expect(m.subject, 'Testing');
      expect(m.keywords, 'pdf test');
    });

    test('handles empty map with defaults', () {
      final m = parseEditorMetadata({});
      expect(m.pageCount, 0);
      expect(m.version, '2.0');
      expect(m.title, '');
      expect(m.author, '');
    });
  });

  group('parseMediaBox', () {
    test('parses rect from map', () {
      final r = parseMediaBox({'x': 0.0, 'y': 0.0, 'width': 612.0, 'height': 792.0});
      expect(r.x, 0.0);
      expect(r.y, 0.0);
      expect(r.width, 612.0);
      expect(r.height, 792.0);
    });

    test('handles int values (coerced to double)', () {
      final r = parseMediaBox({'x': 0, 'y': 0, 'width': 595, 'height': 842});
      expect(r.width, 595.0);
      expect(r.height, 842.0);
    });
  });
}
