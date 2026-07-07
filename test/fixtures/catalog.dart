// The fixture catalog — the single source of truth for every GENERATED
// test fixture.
//
// Each spec names a fixture, says WHY it exists, declares its ground
// truths, and builds it with dart-pdf — the INDEPENDENT producer.
// Fixtures for read/edit/sugar ops are never built by this package's
// own builder: self-feeding lets a builder bug and a reader bug mirror
// each other into green tests.
//
// Consumed ONLY by tool/generate_fixtures.dart (a dev tool). Tests
// import the generated files under generated/ — never this catalog.
//
// Adding a fixture: add a FixtureSpec here, run `make fixtures`, assert
// against the emitted truth constants. Nothing else.

import 'dart:typed_data';

import 'package:pdf/pdf.dart';
import 'package:pdf/widgets.dart' as pw;

/// One generated fixture: identity, rationale, declared truths, builder.
class FixtureSpec {
  /// Creates a spec. [name] is snake_case; the generator derives the
  /// file (`f_<name>.dart`) and identifier (`f<UpperCamel>`) from it.
  const FixtureSpec({
    required this.name,
    required this.why,
    required this.truths,
    required this.build,
  });

  /// snake_case identity.
  final String name;

  /// Why this fixture exists — emitted as the generated file's header.
  final String why;

  /// Declared ground truths — emitted as a const record next to the
  /// bytes. Tests assert against THESE, never against re-derived
  /// engine output. Values: int, double, String, bool, or lists
  /// thereof.
  final Map<String, Object> truths;

  /// Produces the PDF bytes via dart-pdf. Receives the shared
  /// photo-like PNG (the generator builds it once and injects it).
  final Future<Uint8List> Function(Uint8List photoPng) build;
}

pw.Page _textPage(String text, {PdfPageFormat format = PdfPageFormat.a4}) {
  return pw.Page(
    pageFormat: format,
    build: (ctx) => pw.Column(
      crossAxisAlignment: pw.CrossAxisAlignment.start,
      children: [pw.Text(text)],
    ),
  );
}

Future<Uint8List> _saveDoc(pw.Document doc) async =>
    Uint8List.fromList(await doc.save());

/// The full catalog. Order is emission order only — no semantics.
final List<FixtureSpec> catalog = [
  FixtureSpec(
    name: 'blank_a4',
    why:
        'One empty A4 page. The negative-space fixture: extraction '
        'must yield emptiness, image/signature queries must yield '
        'nothing, classification and validation must still answer.',
    truths: {'pages': 1, 'width': 595.0, 'height': 842.0},
    build: (photoPng) async {
      final doc = pw.Document(title: 'Blank A4');
      doc.addPage(
        pw.Page(pageFormat: PdfPageFormat.a4, build: (ctx) => pw.SizedBox()),
      );
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'two_page_markers',
    why:
        'Two pages with distinct markers — the order-proof fixture. '
        'Reorder/move/delete claims are only provable when each page '
        'carries content no other page has.',
    truths: {
      'pages': 2,
      'page0Marker': 'ALPHA UNIQUE',
      'page1Marker': 'BRAVO UNIQUE',
      'title': 'Two Page Markers',
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Two Page Markers');
      doc.addPage(_textPage('ALPHA UNIQUE content on the first page.'));
      doc.addPage(_textPage('BRAVO UNIQUE content on the second page.'));
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'three_page_markers',
    why:
        'Three Letter pages with distinct markers — split / '
        'extractPages / page-restricted-search proofs.',
    truths: {
      'pages': 3,
      'markers': ['MARKER-ONE', 'MARKER-TWO', 'MARKER-THREE'],
      'width': 612.0,
      'height': 792.0,
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Three Page Markers');
      for (final m in ['MARKER-ONE', 'MARKER-TWO', 'MARKER-THREE']) {
        doc.addPage(
          _textPage('$m page content.', format: PdfPageFormat.letter),
        );
      }
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'multisize',
    why:
        'Six pages alternating A5/A3 — splitBySize and media-box '
        'reads need real size variance, not uniform pages.',
    truths: {
      'pages': 6,
      'widths': [420.0, 842.0, 420.0, 842.0, 420.0, 842.0],
      'markerPrefix': 'SIZEPAGE-',
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Multi Size');
      for (var i = 0; i < 6; i++) {
        final big = i.isOdd;
        doc.addPage(
          _textPage(
            'SIZEPAGE-$i on a ${big ? "large A3" : "small A5"} page. '
            '${big ? List.filled(40, "Dense filler content block. ").join() : ""}',
            format: big ? PdfPageFormat.a3 : PdfPageFormat.a5,
          ),
        );
      }
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'outline_chapters',
    why:
        'Three chapters of two pages each with a foreign-produced '
        'outline tree — planSplitByBookmarks / splitByBookmarks must '
        'prove themselves against /Outlines this package did not write.',
    truths: {
      'pages': 6,
      'chapters': ['Chapter One', 'Chapter Two', 'Chapter Three'],
      'startPages': [0, 2, 4],
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Outlined Chapters');
      const chapters = ['Chapter One', 'Chapter Two', 'Chapter Three'];
      for (var c = 0; c < chapters.length; c++) {
        doc.addPage(
          pw.Page(
            pageFormat: PdfPageFormat.a4,
            build: (ctx) => pw.Column(
              crossAxisAlignment: pw.CrossAxisAlignment.start,
              children: [
                pw.Outline(
                  name: 'ch$c',
                  title: chapters[c],
                  child: pw.Text(
                    chapters[c],
                    style: const pw.TextStyle(fontSize: 24),
                  ),
                ),
                pw.Text('Opening page of ${chapters[c]}.'),
              ],
            ),
          ),
        );
        doc.addPage(_textPage('Second page of ${chapters[c]}.'));
      }
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'images',
    why:
        'Three pages, one embedded PNG each — extractImages, render, '
        'and optimizeImages need real raster XObjects from a foreign '
        'writer.',
    truths: {
      'pages': 3,
      'imageCount': 3,
      'imageWidth': 128,
      'imageHeight': 128,
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Image Pages');
      final image = pw.MemoryImage(photoPng);
      for (var i = 0; i < 3; i++) {
        doc.addPage(
          pw.Page(
            pageFormat: PdfPageFormat.a4,
            build: (ctx) => pw.Column(
              crossAxisAlignment: pw.CrossAxisAlignment.start,
              children: [
                pw.Text('IMAGEPAGE-$i'),
                pw.Image(image, width: 128, height: 128),
              ],
            ),
          ),
        );
      }
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'form_fields',
    why:
        'AcroForm fields written by a foreign producer — form fill '
        'and flatten ops must work on forms this package did not '
        'create.',
    truths: {
      'pages': 1,
      'fieldNames': ['fullname', 'agree'],
      'textFieldDefault': 'John Doe',
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Foreign Form');
      doc.addPage(
        pw.Page(
          pageFormat: PdfPageFormat.a4,
          build: (ctx) => pw.Column(
            crossAxisAlignment: pw.CrossAxisAlignment.start,
            children: [
              pw.Text('Application form'),
              pw.TextField(name: 'fullname', value: 'John Doe', width: 200),
              pw.SizedBox(height: 12),
              pw.Checkbox(name: 'agree', value: true),
            ],
          ),
        ),
      );
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'form_fields_de',
    why:
        'German AcroForm from a foreign producer — field name and '
        'default value carry Latin-1-range characters (sharp-s, '
        'umlauts) in whatever text-string encoding dart-pdf emits. '
        'Fill-by-name and flatten must survive them.',
    truths: {
      'pages': 1,
      'fieldName': 'Prüfstraße und Hausnr',
      'textFieldDefault': 'Königsallee 42, München',
    },
    build: (photoPng) async {
      final doc = pw.Document(title: 'Formular');
      doc.addPage(
        pw.Page(
          pageFormat: PdfPageFormat.a4,
          build: (ctx) => pw.Column(
            crossAxisAlignment: pw.CrossAxisAlignment.start,
            children: [
              pw.Text('Anschrift'),
              pw.TextField(
                name: 'Prüfstraße und Hausnr',
                value: 'Königsallee 42, München',
                width: 250,
              ),
            ],
          ),
        ),
      );
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'annotations',
    why:
        'Link annotations from a foreign producer — '
        'flattenAllAnnotations must consume annotation dictionaries '
        'with foreign appearance conventions.',
    truths: {'pages': 1, 'annotCount': 2, 'url': 'https://example.com/interop'},
    build: (photoPng) async {
      final doc = pw.Document(title: 'Linked');
      doc.addPage(
        pw.Page(
          pageFormat: PdfPageFormat.a4,
          build: (ctx) => pw.Column(
            crossAxisAlignment: pw.CrossAxisAlignment.start,
            children: [
              pw.UrlLink(
                child: pw.Text('Visit the site'),
                destination: 'https://example.com/interop',
              ),
              pw.SizedBox(height: 12),
              pw.UrlLink(
                child: pw.Text('Visit it again'),
                destination: 'https://example.com/interop',
              ),
            ],
          ),
        ),
      );
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'unicode',
    why:
        'Diacritics and symbols — extraction honesty beyond ASCII. '
        'A reader that mangles encoding passes every ASCII test. '
        '(No em-dash: dart-pdf\'s base-14 Helvetica cannot encode '
        'U+2014 and silently drops it — producer limitation, proven '
        'against the raw content stream.)',
    truths: {'pages': 1, 'marker': 'Café Münchhausen, naïve façade ©'},
    build: (photoPng) async {
      final doc = pw.Document(title: 'Unicode');
      doc.addPage(_textPage('Café Münchhausen, naïve façade © reserved.'));
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'rotated',
    why:
        'Pages with /Rotate pre-set by a foreign producer — rotation '
        'reads and rotate-composition must respect existing rotation, '
        'not assume zero.',
    truths: {
      'pages': 2,
      'rotations': [90, 180],
      'markerPrefix': 'ROTPAGE-',
    },
    build: (photoPng) async {
      // dart-pdf's widget layer has no /Rotate switch — use its
      // low-level API (still the foreign producer, one layer down).
      final doc = PdfDocument();
      final font = PdfFont.helvetica(doc);
      const rotations = [PdfPageRotation.rotate90, PdfPageRotation.rotate180];
      for (var i = 0; i < rotations.length; i++) {
        final page = PdfPage(
          doc,
          pageFormat: PdfPageFormat.a4,
          rotate: rotations[i],
        );
        page.getGraphics()
          ..setFillColor(PdfColors.black)
          ..drawString(font, 12, 'ROTPAGE-$i content.', 50, 700);
      }
      return Uint8List.fromList(await doc.save());
    },
  ),
  FixtureSpec(
    name: 'hundred_page',
    why:
        'One hundred marked text pages — the mid-weight diet for ops '
        'where 1000 pages would waste suite time.',
    truths: {'pages': 100, 'markerPrefix': 'STRESSPAGE-'},
    build: (photoPng) async {
      final doc = pw.Document(title: 'Hundred Pages');
      for (var i = 0; i < 100; i++) {
        doc.addPage(
          _textPage(
            'STRESSPAGE-$i\nLorem ipsum dolor sit amet, consectetur '
            'adipiscing elit. Ut enim ad minim veniam, quis nostrud '
            'exercitation ullamco laboris. Page seed ${i * 7 + 42}.',
          ),
        );
      }
      return _saveDoc(doc);
    },
  ),
  FixtureSpec(
    name: 'thousand_page',
    why:
        'One thousand marked text pages — the stress diet. Generated '
        'once per machine instead of rebuilt inside every stress '
        'battery on every run.',
    truths: {'pages': 1000, 'markerPrefix': 'STRESSPAGE-'},
    build: (photoPng) async {
      final doc = pw.Document(title: 'Thousand Pages');
      for (var i = 0; i < 1000; i++) {
        doc.addPage(
          _textPage(
            'STRESSPAGE-$i\nLorem ipsum dolor sit amet, consectetur '
            'adipiscing elit. Ut enim ad minim veniam, quis nostrud '
            'exercitation ullamco laboris. Page seed ${i * 7 + 42}.',
          ),
        );
      }
      return _saveDoc(doc);
    },
  ),
];
