// CHARTER — this battery alone proves what a failing DataSource looks like to
// the caller: a read the source throws on ends the operation with
// PdfIoFailed, at open and in the middle of an op, however few reads fail and
// however the engine could have gone on without it; and a file that is
// corrupt, with a source that reads fine, still ends in PdfCorrupted.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// A source whose [failing] reads (1-based, counted from the first) throw.
class _FlakySource extends TestSource {
  _FlakySource(super.data, {this.failing = const {}});

  final Set<int> failing;
  int reads = 0;

  @override
  Uint8List readAt(int offset, int count) {
    if (failing.contains(++reads)) {
      throw StateError('the source failed at read $reads');
    }
    return super.readAt(offset, count);
  }
}

void registerSourceFailureTests(Pdf Function() createPdf, {String? skipMidOp}) {
  group('source failure', () {
    test(
      'a source whose first read fails makes open fail with PdfIoFailed',
      () async {
        final pdf = createPdf();
        final source = _FlakySource(minimalPdf, failing: {1});
        await expectLater(
          pdf.open(PdfInput(source)),
          throwsA(isA<PdfIoFailed>()),
          reason: 'a broken source is not a broken file',
        );
      },
      timeout: t(2),
    );

    test(
      'a source that fails once after the first read fails open too',
      () async {
        // The engine reads the file again to rebuild its xref after a parse
        // error; a source error is not a parse error and ends the open.
        final pdf = createPdf();
        final probe = _FlakySource(fThousandPage);
        await (await pdf.open(PdfInput(probe))).dispose();
        expect(probe.reads, greaterThan(1));

        for (var read = 2; read <= probe.reads; read++) {
          final source = _FlakySource(fThousandPage, failing: {read});
          await expectLater(
            pdf.open(PdfInput(source)),
            throwsA(isA<PdfIoFailed>()),
            reason: 'the source threw on read $read of ${probe.reads}',
          );
        }
      },
      timeout: t(20),
    );

    test(
      'a source whose read fails once mid-op fails that op with PdfIoFailed',
      skip: skipMidOp,
      () async {
        final pdf = createPdf();
        final source = _FlakySource(fThousandPage);
        final doc = await pdf.open(PdfInput(source));
        final afterOpen = source.reads;
        await doc.text(pages: const PdfPages.single(900));
        final lastRead = source.reads;
        expect(
          lastRead,
          greaterThan(afterOpen),
          reason: 'the op must read the source for the failure to land in it',
        );

        for (var read = afterOpen + 1; read <= lastRead; read++) {
          final flaky = _FlakySource(fThousandPage, failing: {read});
          final failing = await pdf.open(PdfInput(flaky));
          await expectLater(
            failing.text(pages: const PdfPages.single(900)),
            throwsA(isA<PdfIoFailed>()),
            reason: 'the source threw on read $read',
          );
          await failing.dispose();
        }
        await doc.dispose();
      },
      timeout: t(20),
    );

    test(
      'a corrupt file read by a sound source is still PdfCorrupted',
      () async {
        final pdf = createPdf();
        for (final bytes in [garbageBytes, emptyBytes, brokenPdf]) {
          final source = _FlakySource(bytes);
          await expectLater(
            pdf.open(PdfInput(source)),
            throwsA(isA<PdfCorrupted>()),
          );
        }
      },
      timeout: t(5),
    );
  });
}
