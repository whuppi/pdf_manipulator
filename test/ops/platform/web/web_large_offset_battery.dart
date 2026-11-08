// CHARTER — this battery alone proves: on web, a source read past 4 GiB
// returns the bytes at that offset, not the bytes at the offset modulo 2^32.
//
// Diet: one source of 4 GiB + 64 MiB that holds no such buffer. `readAt`
// synthesizes its bytes: the `%PDF-1.7` header, whitespace filler, the
// objects of a one-page PDF at 4 GiB + 16, more filler, then the xref and
// trailer at the very end. Every offset the xref and `startxref` name is
// past 4 GiB, and the filler a wrapped offset lands in holds no object, so a
// read at the wrapped offset cannot parse. The page's text proves it.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../../harness/extract_text.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

const _fourGiB = 4294967296;
const _marker = 'PASTFOURGIB 4294967296';

/// A source of 4 GiB + 64 MiB whose bytes are made on `readAt`.
class PastFourGiBSource implements DataSource {
  PastFourGiBSource({int extra = 64 * 1024 * 1024})
    : length = _fourGiB + extra {
    const header = '%PDF-1.7\n';
    final objects = [
      '<< /Type /Catalog /Pages 2 0 R >>',
      '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
      '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>',
      () {
        const content = 'BT /F1 18 Tf 72 720 Td ($_marker) Tj ET';
        return '<< /Length ${content.length} >>\nstream\n$content\nendstream';
      }(),
      '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    ];

    _bodyStart = _fourGiB + 16;
    final body = StringBuffer();
    final offsets = <int>[];
    for (var i = 0; i < objects.length; i++) {
      offsets.add(_bodyStart + body.length);
      body.write('${i + 1} 0 obj\n${objects[i]}\nendobj\n');
    }
    _body = _latin1(body.toString());

    // The xref is the last bytes of the source, so its offset is known from
    // the length alone.
    final tail = StringBuffer('xref\n0 ${objects.length + 1}\n')
      ..write('0000000000 65535 f \n');
    for (final offset in offsets) {
      tail.write('${offset.toString().padLeft(10, '0')} 00000 n \n');
    }
    tail.write(
      'trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\n'
      'startxref\n',
    );
    final fixed = tail.length + 'XXXXXXXXXX\n%%EOF\n'.length;
    final xrefStart = length - fixed;
    tail.write('${xrefStart.toString().padLeft(10, '0')}\n%%EOF\n');
    _tail = _latin1(tail.toString());
    _header = _latin1(header);
    assert(_bodyStart + _body.length < length - _tail.length);
    assert(xrefStart + _tail.length == length);
  }

  @override
  final int length;

  late final Uint8List _header;
  late final Uint8List _body;
  late final Uint8List _tail;
  late final int _bodyStart;

  /// The highest offset any read began at.
  int highestRead = 0;

  /// Every read, as (offset, count), in order.
  final reads = <(int, int)>[];

  static Uint8List _latin1(String s) => Uint8List.fromList(s.codeUnits);

  @override
  Uint8List readAt(int offset, int count) {
    if (count > maxReadChunk) {
      throw StateError('O(1) SOURCE violation: readAt of $count bytes');
    }
    if (offset > highestRead) highestRead = offset;
    reads.add((offset, count));
    final end = offset + count > length ? length : offset + count;
    if (offset >= end) return Uint8List(0);
    final out = Uint8List(end - offset);
    final tailStart = length - _tail.length;
    for (var i = 0; i < out.length; i++) {
      final p = offset + i;
      if (p < _header.length) {
        out[i] = _header[p];
      } else if (p >= _bodyStart && p < _bodyStart + _body.length) {
        out[i] = _body[p - _bodyStart];
      } else if (p >= tailStart) {
        out[i] = _tail[p - tailStart];
      } else {
        // Filler a parser skips between its parts.
        out[i] = p % 64 == 63 ? 0x0A : 0x20;
      }
    }
    return out;
  }

  @override
  void close() {}
}

/// [skip] names why a mode cannot run it.
void registerWebLargeOffsetTests(Pdf Function() createPdf, {String? skip}) {
  group('web large offsets', () {
    test(
      'a source read past 4 GiB returns the bytes at that offset',
      () async {
        final source = PastFourGiBSource();
        final doc = await createPdf().open(PdfInput(source));
        try {
          final text = await extractTextToString(doc);
          expect(text, contains(_marker));
          expect(source.highestRead, greaterThanOrEqualTo(_fourGiB));
        } finally {
          await doc.dispose();
        }
      },
      timeout: t(60),
      skip: skip,
    );

    test(
      'a read with a full window left asks for a full window when the '
      'length passes 4 GiB by less than one',
      () async {
        // 4 GiB + 4 KiB left at offset 0 is 4 KiB when narrowed to 32 bits.
        final source = PastFourGiBSource(extra: 4096);
        final doc = await createPdf().open(PdfInput(source));
        try {
          expect(await extractTextToString(doc), contains(_marker));
          final short = [
            for (final (offset, count) in source.reads)
              if (offset + maxReadChunk <= source.length &&
                  count != maxReadChunk)
                (offset, count),
          ];
          expect(source.reads, isNotEmpty);
          expect(short, isEmpty);
        } finally {
          await doc.dispose();
        }
      },
      timeout: t(60),
      skip: skip,
    );
  });
}
