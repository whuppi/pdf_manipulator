// CHARTER — this battery alone proves: what a save writes for pages merged
// in by `mergeFrom` — their content and image bytes, one object per id with
// every id in the xref, a link that still names the page it sits on, the
// merged form fields and /DR without the merged outline, no source page
// tree behind a link to an unmerged page, and an object a dropped merged
// page met first still written for the kept pages that share it. That the
// merged text, links and fields show to a reader is the editor battery's
// claim, not this one's.
//
// Diet: handwritten micro fixtures. The claims are about the saved file's
// own objects, so they read a plain save (no object streams, no compression)
// back by its xref: a decoded read cannot tell a copy from a re-encode.

import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/extract_text.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// A save with nothing packed or compressed.
const _plain = PdfSaveOptions.fullRewrite(deflate: false, objectStreams: false);

String _latin1(Uint8List bytes) =>
    String.fromCharCodes(bytes); // bytegrep-exempt: raw file text

/// The objects of a plain save, read by the xref table the save wrote.
class _Raw {
  // bytegrep-exempt: the claims are about the saved file's own objects.
  _Raw(this.bytes) : text = _latin1(bytes) {
    final start = RegExp(r'startxref\s+(\d+)').allMatches(text).last;
    var at = int.parse(start[1]!);
    expect(text.startsWith('xref', at), isTrue, reason: 'a classic xref table');
    final header = RegExp(r'xref\s+0 (\d+)\s+').matchAsPrefix(text, at)!;
    at = header.end;
    for (var id = 0; id < int.parse(header[1]!); id++) {
      final entry = text.substring(at, at + 20);
      at += 20;
      if (entry[17] == 'n') offsets[id] = int.parse(entry.substring(0, 10));
    }
  }

  final Uint8List bytes;
  final String text;

  /// Object number to byte offset, for each object the xref lists in use.
  final offsets = <int, int>{};

  /// The number of each `N 0 obj` header in the file, in file order.
  List<int> get headerIds => [
    for (final m in RegExp(r'^(\d+) 0 obj$', multiLine: true).allMatches(text))
      int.parse(m[1]!),
  ];

  /// Every object written, by number, as the text from its header to its
  /// `endobj`.
  Map<int, String> get objects => {for (final id in headerIds) id: object(id)};

  String object(int id) {
    final from = text.indexOf(RegExp('^$id 0 obj\$', multiLine: true));
    return text.substring(from, text.indexOf('endobj', from));
  }

  /// The objects whose text matches [pattern].
  Map<int, String> matching(Pattern pattern) => {
    for (final e in objects.entries)
      if (e.value.contains(pattern)) e.key: e.value,
  };

  int count(Pattern pattern) => pattern.allMatches(text).length;

  /// Whether [needle] is among the file's bytes.
  bool holds(List<int> needle) {
    outer:
    for (var i = 0; i <= bytes.length - needle.length; i++) {
      for (var j = 0; j < needle.length; j++) {
        if (bytes[i + j] != needle[j]) continue outer;
      }
      return true;
    }
    return false;
  }
}

/// A page object, not a page tree node.
final _pageObject = RegExp(r'/Type\s*/Page(?![A-Za-z])');
final _pagesObject = RegExp(r'/Type\s*/Pages(?![A-Za-z])');
final _helvetica = RegExp(r'/BaseFont\s*/Helvetica');

void registerMergeTests(Pdf Function() createPdf) {
  group('merge', () {
    /// Merges [source] (optionally only [pages]) into [base], then saves with
    /// [edit] applied in between.
    Future<Uint8List> mergedSave(
      Uint8List base,
      Uint8List source, {
      PdfPages? pages,
      Future<void> Function(PdfEditor editor)? edit,
    }) async {
      final editor = await createPdf().edit(src(base));
      await editor.mergeFrom(src(source), pages: pages);
      await edit?.call(editor);
      final sink = TestSink();
      await editor.save(sink, options: _plain);
      await editor.dispose();
      return sink.takeBytes();
    }

    Future<List<String>> pageTexts(Uint8List bytes) async {
      final doc = await createPdf().open(src(bytes));
      try {
        return [
          for (var i = 0; i < doc.pageCount; i++)
            (await extractTextToString(doc, pages: PdfPages.single(i))).trim(),
        ];
      } finally {
        await doc.dispose();
      }
    }

    test(
      'a merged page\'s content and image are copied byte for byte',
      () async {
        final raw = _Raw(await mergedSave(minimalPdf, mergeSourcePdf));
        // bytegrep-exempt: a decoded read cannot tell a copy from a re-encode.
        expect(raw.holds(utf8.encode(mergeSourceContent)), isTrue);
        expect(raw.holds(mergeSourceImage), isTrue);
      },
      timeout: t(1),
    );

    test('a merged save writes one object per id and lists each in the '
        'xref', () async {
      final raw = _Raw(await mergedSave(minimalPdf, mergeSourcePdf));
      final ids = raw.headerIds;
      expect(ids.toSet(), hasLength(ids.length), reason: 'ids written: $ids');
      for (final id in ids) {
        expect(raw.offsets, contains(id), reason: 'object $id is in the xref');
        expect(
          raw.text.startsWith('$id 0 obj', raw.offsets[id]!),
          isTrue,
          reason: 'the xref offset of object $id',
        );
      }
    }, timeout: t(1));

    test('a merged link keeps its destination and names the merged '
        'page', () async {
      final raw = _Raw(await mergedSave(minimalPdf, mergeSourcePdf));
      final pages = raw.matching(_pageObject);
      expect(pages, hasLength(2));
      final mergedPage = pages.entries.singleWhere(
        (e) => e.value.contains('/Annots'),
      );
      final link = raw.matching(RegExp(r'/Subtype\s*/Link')).values.single;
      expect(
        RegExp(r'/P\s+(\d+) 0 R').firstMatch(link)![1],
        '${mergedPage.key}',
      );
      expect(
        RegExp(r'/Dest\s*\[\s*(\d+) 0 R').firstMatch(link)![1],
        '${mergedPage.key}',
        reason: 'a destination names the merged page',
      );
    }, timeout: t(1));

    test('a merge imports no outline and lists the merged fields', () async {
      final bytes = await mergedSave(minimalPdf, mergeSourcePdf);
      final doc = await createPdf().open(src(bytes));
      final names = [for (final f in await doc.formFields()) f.name];
      await doc.dispose();
      expect(names, ['mergedfield']);

      final raw = _Raw(bytes);
      final catalog = raw.matching(RegExp(r'/Type\s*/Catalog')).values.single;
      expect(
        catalog,
        isNot(contains('/Outlines')),
        reason: 'the first document\'s outline is kept, not the merged one',
      );
      // The AcroForm is the catalog's own or a reference to its object.
      final form = RegExp(r'/AcroForm\s+(\d+) 0 R').firstMatch(catalog);
      expect(
        form == null ? catalog : raw.object(int.parse(form[1]!)),
        contains('/DR'),
        reason: 'the merged /DR is carried',
      );
    }, timeout: t(1));

    group('with merged pages after source pages', () {
      final base = markerPagesPdf(['S0', 'S1']);
      final merged = markerPagesPdf(['M0', 'M1', 'M2']);

      test('dropped merged pages are not written and a shared object is '
          'written once', () async {
        final bytes = await mergedSave(
          base,
          merged,
          edit: (editor) => editor.keepPages([3]),
        );
        expect(await pageTexts(bytes), ['M1']);
        final raw = _Raw(bytes);
        // bytegrep-exempt: the claim is which page content the file holds.
        for (final dropped in ['(S0)', '(S1)', '(M0)', '(M2)']) {
          expect(raw.holds(utf8.encode(dropped)), isFalse, reason: dropped);
        }
        expect(raw.holds(utf8.encode('(M1)')), isTrue);
        expect(
          raw.count(_helvetica),
          1,
          reason: 'the font the pages share is written once',
        );
      }, timeout: t(1));

      test('an object only a dropped merged page met first is written for a '
          'kept page', () async {
        // The merged font is first met by M0, which is dropped; M1 and M2
        // still need it.
        final bytes = await mergedSave(
          base,
          merged,
          edit: (editor) => editor.deletePages(const PdfPages.single(2)),
        );
        expect(await pageTexts(bytes), ['S0', 'S1', 'M1', 'M2']);
        final raw = _Raw(bytes);
        // bytegrep-exempt: the claim is which page content the file holds.
        expect(raw.holds(utf8.encode('(M0)')), isFalse);
        expect(
          raw.count(_helvetica),
          2,
          reason: 'one font for the source pages, one for the merged pages',
        );
      }, timeout: t(1));
    });

    test('a link to a page that was not merged imports no page tree', () async {
      final raw = _Raw(
        await mergedSave(minimalPdf, linkedPairPdf, pages: PdfPages.list([0])),
      );
      final ids = raw.headerIds;
      expect(ids.toSet(), hasLength(ids.length), reason: 'ids written: $ids');
      expect(
        raw.matching(_pagesObject),
        hasLength(1),
        reason: 'only the output\'s own page tree',
      );
      expect(
        raw.matching(_pageObject),
        hasLength(2),
        reason: 'the unmerged source page is not imported',
      );
      final link = raw.matching(RegExp(r'/Subtype\s*/Link')).values.single;
      expect(
        link,
        contains(RegExp(r'/Dest\s*\[\s*null\b')),
        reason: 'a destination page that was left out is null',
      );
    }, timeout: t(1));
  });
}
