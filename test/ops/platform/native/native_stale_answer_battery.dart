// Native-only: the late answer of a cancelled read never reaches the next
// request on the same held channel.
//
// A document keeps its source's channel open between jobs. Cancel a job
// while the source is still serving its read, start another job on the same
// document, and let the first read finish while the second is waiting: the
// engine must not take the first read's bytes as the second's answer. Only
// the native transport has a shared buffer a late answer can land in, so no
// web battery can see it.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

const _pageCount = 300;

Future<Uint8List> _build(Pdf pdf) async {
  final builder = await pdf.build();
  for (var i = 0; i < _pageCount; i++) {
    final page = await builder.addPage(PdfPageSize.points(612, 792));
    await page.text('stale answer page $i');
  }
  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// A source that serves honestly until armed. Armed, its first read takes
/// [lateDelay] and answers with zeros; its second read waits for
/// [releaseSecond] and answers truthfully; every later read answers at once.
class _ScriptedSource implements DataSource {
  _ScriptedSource(this._bytes, {required this.lateDelay});

  final Uint8List _bytes;
  final Duration lateDelay;

  bool armed = false;
  int _armedReads = 0;

  final firstRead = Completer<void>();
  final secondRead = Completer<void>();
  final lateAnswerGiven = Completer<void>();
  final _releaseSecond = Completer<void>();

  void releaseSecond() {
    if (!_releaseSecond.isCompleted) _releaseSecond.complete();
  }

  @override
  int get length => _bytes.length;

  Uint8List _true(int offset, int count) {
    if (offset >= _bytes.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _bytes.length);
    return Uint8List.sublistView(_bytes, offset, end);
  }

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    if (!armed) return _true(offset, count);
    final nth = ++_armedReads;
    if (nth == 1) {
      firstRead.complete();
      await Future<void>.delayed(lateDelay);
      final wrong = Uint8List(_true(offset, count).length);
      // Resolves into the transport; the signal below fires once the
      // transport has had its turn to act on it.
      scheduleMicrotask(() async {
        await Future<void>.delayed(const Duration(milliseconds: 200));
        lateAnswerGiven.complete();
      });
      return wrong;
    }
    if (nth == 2) {
      secondRead.complete();
      await _releaseSecond.future;
    }
    return _true(offset, count);
  }

  @override
  void close() {}
}

void registerNativeStaleAnswerTests() {
  test('the late answer of a cancelled read is not the next request\'s '
      'answer', () async {
    final pdf = Pdf();
    final bytes = await _build(pdf);

    final expectedDoc = await pdf.open(PdfInput(src(bytes)));
    final expectedSink = MemorySink();
    await expectedDoc.extractText(
      expectedSink,
      pages: const PdfPages.single(0),
    );
    final expected = expectedSink.takeString();
    expect(expected, contains('stale answer page 0'));
    await expectedDoc.dispose();

    final source = _ScriptedSource(
      bytes,
      lateDelay: const Duration(seconds: 2),
    );
    final doc = await pdf.open(PdfInput(source));

    source.armed = true;
    final first = doc.extractText(MemorySink(), pages: const PdfPages.all());
    await source.firstRead.future; // the engine is parked inside read 1
    first.cancel();
    await expectLater(first, throwsA(isA<PdfCancelled>()));

    final sink = MemorySink();
    final second = doc.extractText(sink, pages: const PdfPages.single(0));
    var secondDone = false;
    unawaited(second.then((_) => secondDone = true, onError: (_) {}));
    await source.secondRead.future; // the engine now waits on read 2

    await source.lateAnswerGiven.future; // read 1 resolves into read 2's slot
    expect(
      secondDone,
      isFalse,
      reason: 'the second job took the cancelled read\'s answer',
    );

    source.releaseSecond();
    await second;
    expect(
      sink.takeString(),
      expected,
      reason: 'the second job must get its own bytes',
    );
    await doc.dispose();
    await pdf.dispose();
  }, timeout: t(20));
}
