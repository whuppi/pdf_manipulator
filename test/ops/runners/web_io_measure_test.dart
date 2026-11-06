// Measurement, not a test of behaviour: times open + extract and merge of one
// large generated PDF in the Asyncify and OPFS modes (OPFS includes its
// pre-copy of every input). Skipped unless a size is given:
//
//   dart test test/ops/runners/web_io_measure_test.dart -p chrome \
//       --dart2js-args=-DMEASURE_MB=300
//
// Each line is `MEASURE mode op ms`. The PDF is built once in memory, as
// uncompressed text pages, so extraction does real work on every page.

@TestOn('browser')
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../harness/test_source_sink.dart';

const _megabytes = int.fromEnvironment('MEASURE_MB');
const _extractPages = 50;

/// A text PDF of about [megabytes] MB: pages of 48 KiB of content each.
Uint8List _bigPdf(int megabytes) {
  const pageBytes = 48 * 1024;
  final pages = megabytes * 1024 * 1024 ~/ pageBytes;
  final out = BytesBuilder(copy: false);
  final offsets = <int>[];
  var length = 0;
  void add(List<int> bytes) {
    out.add(bytes);
    length += bytes.length;
  }

  void object(int number, String body) {
    offsets.add(length);
    add(ascii.encode('$number 0 obj\n$body\nendobj\n'));
  }

  add(ascii.encode('%PDF-1.7\n'));
  object(1, '<< /Type /Catalog /Pages 2 0 R >>');
  final kids = [for (var i = 0; i < pages; i++) '${4 + 2 * i} 0 R'].join(' ');
  object(2, '<< /Type /Pages /Kids [$kids] /Count $pages >>');
  object(3, '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
  final line = ascii.encode(
    'BT /F1 10 Tf 40 700 Td (The quick brown fox jumps over the lazy dog) Tj ET\n',
  );
  final stream = BytesBuilder(copy: false);
  for (var n = 0; n < pageBytes ~/ line.length; n++) {
    stream.add(line);
  }
  final content = stream.takeBytes();
  for (var i = 0; i < pages; i++) {
    final page = 4 + 2 * i;
    object(
      page,
      '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] '
      '/Resources << /Font << /F1 3 0 R >> >> /Contents ${page + 1} 0 R >>',
    );
    offsets.add(length);
    add(
      ascii.encode(
        '${page + 1} 0 obj\n<< /Length ${content.length} >>\nstream\n',
      ),
    );
    add(content);
    add(ascii.encode('\nendstream\nendobj\n'));
  }
  final xref = length;
  final count = offsets.length + 1;
  final table = StringBuffer('xref\n0 $count\n0000000000 65535 f \n');
  for (final o in offsets) {
    table.write('${o.toString().padLeft(10, '0')} 00000 n \n');
  }
  add(ascii.encode(table.toString()));
  add(
    ascii.encode(
      'trailer\n<< /Size $count /Root 1 0 R >>\nstartxref\n$xref\n%%EOF\n',
    ),
  );
  return out.takeBytes();
}

class _CountingSink implements DataSink {
  int bytes = 0;

  @override
  FutureOr<void> write(Uint8List chunk) => bytes += chunk.length;

  @override
  FutureOr<void> close() {}

  @override
  FutureOr<void> abort(Object reason) {}
}

void main() {
  late Uint8List big;
  late String workerUrl;

  setUpAll(() async {
    if (_megabytes == 0) return;
    final channel = spawnHybridUri(
      '/test/harness/asset_server.dart',
      stayAlive: true,
    );
    final port = (await channel.stream.first as double).toInt();
    workerUrl = 'http://localhost:$port/web_assets/executor_worker.js';
    big = _bigPdf(_megabytes);
  });

  // Each mode twice, alternating, so a warm-up shows as the first pair.
  for (final mode in [
    PdfIoMode.asyncify,
    PdfIoMode.opfs,
    PdfIoMode.asyncify,
    PdfIoMode.opfs,
  ]) {
    test(
      '${mode.name}: open + extract, then merge, of a $_megabytes MB PDF',
      () async {
        final pdf = Pdf(
          config: PdfConfig(webWorkerUrl: workerUrl, webIoMode: mode),
        );
        final clock = Stopwatch()..start();
        var reads = 0;
        final doc = await pdf.open(
          PdfInput(TestSource(big, onRead: (_, _) => reads++)),
        );
        final extracted = _CountingSink();
        await doc.extractText(
          extracted,
          pages: PdfPages.range(0, _extractPages),
        );
        final openExtract = clock.elapsedMilliseconds;
        await doc.dispose();

        clock
          ..reset()
          ..start();
        final merged = _CountingSink();
        await pdf.merge([
          PdfInput(TestSource(big)),
          PdfInput(TestSource(big)),
        ], merged);
        final merge = clock.elapsedMilliseconds;
        print(
          'MEASURE ${mode.name} bytes=${big.length} '
          'open+extract=${openExtract}ms (reads=$reads) merge=${merge}ms '
          'extracted=${extracted.bytes} merged=${merged.bytes}',
        );
        await pdf.dispose();
      },
      timeout: Timeout(const Duration(minutes: 30)),
      skip: _megabytes == 0 ? 'pass --dart2js-args=-DMEASURE_MB=<size>' : false,
    );
  }
}
