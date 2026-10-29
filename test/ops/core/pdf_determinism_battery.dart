// CHARTER — this battery alone proves: the same input with the same options
// saves to the same bytes, on every save path. Compress at each preset, full
// rewrite (images reduced, watermarks overlaid), incremental, merge, extract,
// a kept encryption and sign. Each test runs the operation twice; every run
// opens a fresh engine editor, and each editor's hash maps carry their own
// random seed, so an output order that follows a hash map shows up as a byte
// difference.
//
// Diet: one built fixture with 60 pages, each holding a distinct image, so
// the saves stage 60 modified image objects and write 60 xref entries from
// them. The chance two random orders of 60 objects coincide is 1/60!,
// about 1.2e-82: an unsorted write cannot pass by luck.
//
// The two exceptions are named, not hidden. An encrypted save draws a random
// AES IV per string and stream (ISO 32000 calls the IV a 16-byte random
// number), so the kept-encryption test decrypts both outputs and compares
// those. `sign` reads the clock once per call, so the sign test pairs two
// runs that fall in one second (RSA PKCS#1 v1.5 itself is deterministic).

import 'dart:typed_data';

import 'package:image/image.dart' as img;
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

const _pages = 60;

/// A 256² image no other seed shares, so the writer cannot merge two pages'
/// images into one object. Noise keeps every preset from returning the
/// source untouched.
Uint8List _image(int seed) {
  final image = img.Image(width: 256, height: 256);
  var state = 0x9E3779B1 ^ (seed * 0x85EBCA6B);
  for (final pixel in image) {
    state = (state * 1664525 + 1013904223) & 0xFFFFFFFF;
    pixel
      ..r = state & 0xFF
      ..g = (state >> 8) & 0xFF
      ..b = (state >> 16) & 0xFF;
  }
  return Uint8List.fromList(img.encodePng(image));
}

/// [_pages] Letter pages, each with a heading and one dense image.
Future<Uint8List> _fixture(
  Pdf pdf, {
  int pages = _pages,
  int offset = 0,
}) async {
  final builder = await pdf.build();
  for (var i = 0; i < pages; i++) {
    final page = await builder.addLetterPage();
    await page.at(72, 720);
    await page.text('page ${i + offset}');
    await page.image(
      src(_image(i + offset)),
      const PdfRect(x: 72, y: 600, width: 40, height: 40),
    );
    await page.done();
  }
  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

Future<Uint8List> _save(
  Pdf pdf,
  Uint8List input,
  Future<void> Function(PdfEditor editor) edit, {
  PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  String? password,
}) async {
  final editor = await pdf.edit(src(input), password: password);
  await edit(editor);
  final sink = TestSink();
  await editor.save(sink, options: options);
  await editor.dispose();
  return sink.takeBytes();
}

void _expectSame(Uint8List first, Uint8List second) {
  expect(second.length, first.length, reason: 'the two saves differ in size');
  expect(second, orderedEquals(first), reason: 'the two saves differ in bytes');
}

void registerDeterminismTests(Pdf Function() createPdf) {
  group('determinism', () {
    late Uint8List fixture;
    late Uint8List other;

    setUpAll(() async {
      final pdf = createPdf();
      fixture = await _fixture(pdf);
      other = await _fixture(pdf, pages: 20, offset: 1000);
    });

    for (final preset in <String, PdfImagePolicy>{
      'lossless': PdfImagePolicy.lossless,
      'print': PdfImagePolicy.print,
      'ebook': PdfImagePolicy.ebook,
      'screen': PdfImagePolicy.screen,
    }.entries) {
      test('compress at ${preset.key} saves the same bytes twice', () async {
        final pdf = createPdf();
        Future<Uint8List> run() async {
          final sink = TestSink();
          await pdf.compress(src(fixture), sink, images: preset.value);
          return sink.takeBytes();
        }

        final first = await run();
        expect(first, isNotEmpty);
        _expectSame(first, await run());
      }, timeout: t(30));
    }

    test('a full rewrite with reduced images and overlays saves the same '
        'bytes twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() => _save(pdf, fixture, (editor) async {
        await editor.reduceImages(PdfImagePolicy.screen);
        await editor.setTitle('determinism');
        for (var page = 0; page < _pages; page += 3) {
          await editor.addWatermark(page, 'DRAFT');
        }
      });

      _expectSame(await run(), await run());
    }, timeout: t(30));

    test('a full rewrite without garbage collection saves the same bytes '
        'twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() => _save(
        pdf,
        fixture,
        (editor) => editor.reduceImages(PdfImagePolicy.ebook),
        options: const PdfSaveOptions.fullRewrite(garbageCollect: false),
      );

      _expectSame(await run(), await run());
    }, timeout: t(30));

    test('an incremental save saves the same bytes twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() => _save(
        pdf,
        fixture,
        (editor) => editor.reduceImages(PdfImagePolicy.screen),
        options: const PdfSaveOptions.incremental(),
      );

      _expectSame(await run(), await run());
    }, timeout: t(30));

    test('a merge saves the same bytes twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() async {
        final sink = TestSink();
        await pdf.merge([src(fixture), src(other), src(fixture)], sink);
        return sink.takeBytes();
      }

      _expectSame(await run(), await run());
    }, timeout: t(30));

    // The writer registers the 12 Standard-14 Latin fonts on every document
    // and writes them from a hash map: the chance two random orders agree is
    // 1/12!, about 2e-9.
    test('a built document saves the same bytes twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() async {
        final builder = await pdf.build();
        final page = await builder.addLetterPage();
        await page.at(72, 720);
        await page.text('determinism');
        await page.done();
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        return sink.takeBytes();
      }

      _expectSame(await run(), await run());
    }, timeout: t(30));

    // Each of 40 radio groups has a parent field and a named kid field that
    // share one widget object. Setting the parent's value stages the widget
    // with a new /AS; changing the kid's tooltip stages the same widget again
    // from the source. Whichever flushes last wins, so each group is a coin
    // flip per save: the chance two random orders agree on all 40 is 2^-40,
    // about 1e-12.
    test('form fields that share a widget save the same bytes twice', () async {
      final pdf = createPdf();
      const groups = 40;
      final form = radioGroupsForm(groups);
      Future<Uint8List> run() => _save(pdf, form, (editor) async {
        for (var i = 0; i < groups; i++) {
          await editor.setFormFieldValue('g$i', 'On');
          await editor.setFormFieldTooltip('g$i.k', 'tip $i');
        }
      });

      _expectSame(await run(), await run());
    }, timeout: t(30));

    test('a merge of selected pages saves the same bytes twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() => _save(pdf, fixture, (editor) async {
        await editor.mergeFrom(src(other), pages: [0, 2, 4, 6, 8, 10]);
        await editor.reduceImages(PdfImagePolicy.ebook);
      });

      _expectSame(await run(), await run());
    }, timeout: t(30));

    test('extracting pages saves the same bytes twice', () async {
      final pdf = createPdf();
      Future<Uint8List> run() async {
        final sink = TestSink();
        await pdf.extractPages(
          src(fixture),
          sink,
          pages: [for (var i = 0; i < _pages; i += 2) i],
        );
        return sink.takeBytes();
      }

      _expectSame(await run(), await run());
    }, timeout: t(30));

    test(
      'a kept encryption saves the same bytes twice once decrypted',
      () async {
        final pdf = createPdf();
        const config = PdfEncryptionConfig(
          ownerPassword: 'owner-pw',
          userPassword: 'user-pw',
        );
        final encrypted = TestSink();
        await pdf.encrypt(src(fixture), encrypted, encryption: config);
        final source = encrypted.takeBytes();

        Future<Uint8List> run() async {
          final kept = await _save(
            pdf,
            source,
            (editor) => editor.reduceImages(PdfImagePolicy.screen),
            password: 'user-pw',
          );
          final plain = TestSink();
          await pdf.decrypt(src(kept), plain, password: 'user-pw');
          return plain.takeBytes();
        }

        _expectSame(await run(), await run());
      },
      timeout: t(30),
    );

    test('sign saves the same bytes twice within one second', () async {
      final pdf = createPdf();
      Future<(Uint8List, int)> run() async {
        final sink = TestSink();
        final second = DateTime.now().millisecondsSinceEpoch ~/ 1000;
        await pdf.sign(
          src(fixture),
          sink,
          credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
        );
        return (sink.takeBytes(), second);
      }

      // The signing time is the clock's second; retry until two runs share
      // one, which a short document does on almost every attempt.
      for (var attempt = 0; attempt < 10; attempt++) {
        final (first, startedAt) = await run();
        final (second, startedAgain) = await run();
        final endedAt = DateTime.now().millisecondsSinceEpoch ~/ 1000;
        if (startedAt == startedAgain && startedAgain == endedAt) {
          _expectSame(first, second);
          return;
        }
      }
      fail('no two sign runs fell in one second');
    }, timeout: t(60));
  });
}
