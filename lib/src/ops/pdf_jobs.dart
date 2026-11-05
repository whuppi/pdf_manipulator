// Jobs — the doors of Pdf that open several handles, apply a policy or loop
// a builder step. Every other edit is a member of PdfEdits and runs through
// `rewrite` or `pdf.editOnce`.
//
// Every handle a job opens goes through `_own`, which closes it after
// success, error and cancel. A sink a job obtains from a factory is the
// library's and is closed after its item, on error and on cancel.

part of 'pdf.dart';

/// Opens [input] for the length of [run] and releases it afterwards, on
/// success, error and cancel. [output], when the job has one, is aborted if
/// the input cannot be opened.
PdfTask<void> _withInput(
  PdfInput input,
  DataSink output,
  PdfTask<void> Function(DataSource source) run,
) => PdfTask.group((hook) async {
  try {
    final lease = await input.opened;
    try {
      await hook.guard(run(lease.source));
    } finally {
      await lease.release();
    }
  } catch (error) {
    await hook.abortSinks([output], error);
    rethrow;
  }
});

/// A handle closer: [dispose] then [release], in that order, even when
/// [dispose] throws.
Future<void> Function() _closing(
  Future<void> Function() dispose,
  Future<void> Function() release,
) => () async {
  try {
    await dispose();
  } finally {
    await release();
  }
};

PdfTask<void> _merge(
  Pdf pdf,
  List<PdfInput> inputs,
  DataSink output,
  PdfSave save,
) {
  if (inputs.isEmpty) {
    throw ArgumentError.value(inputs, 'inputs', 'must not be empty');
  }
  return pdf.rewrite(inputs.first, output, (editor) async {
    for (var i = 1; i < inputs.length; i++) {
      await editor.appendPages(inputs[i]);
    }
  }, save: save);
}

PdfTask<List<PdfSplitPart>> _split(
  Pdf pdf,
  PdfInput input,
  FutureOr<DataSink> Function(PdfSplitPart part) sinkFor,
  PdfSplitRule by,
) {
  return _own(pdf.edit(input), (editor, hook) async {
    final handle = editor._handle;
    final parts = <PdfSplitPart>[];

    Future<void> deliver(PdfSplitPart part) async {
      final sink = await sinkFor(part);
      try {
        await handle.extractPages(_pageRun(part.start, part.end), sink);
        // A cancel that lands as the part's last write does is the part's:
        // it is not delivered.
        if (hook.isCancelled) throw const PdfCancelled();
      } catch (error) {
        // The op aborted the sink if it ran; the hook aborts it once.
        await hook.abortSinks([sink], error);
        rethrow;
      } finally {
        await sink.close();
      }
      parts.add(part);
    }

    final total = await handle.pageCount();
    switch (by) {
      case PdfSplitEvery(:final pages):
        for (var start = 0; start < total; start += pages) {
          await deliver(
            PdfSplitPart(
              index: parts.length,
              start: start,
              end: min(start + pages, total),
            ),
          );
        }
      case PdfSplitMaxBytes(:final bytes):
        for (var start = 0; start < total;) {
          final count = await _pagesThatFit(handle, start, total, bytes);
          await deliver(
            PdfSplitPart(index: parts.length, start: start, end: start + count),
          );
          start += count;
        }
      case PdfSplitBookmarks():
        final ranges = await hook.guard(
          _own(
            pdf.open(input),
            (doc, planHook) => planHook.guard(doc.bookmarkRanges()),
          ),
        );
        for (final range in ranges) {
          await deliver(
            PdfSplitPart(
              index: parts.length,
              start: range.start,
              end: range.end,
              title: range.title,
            ),
          );
        }
    }
    return parts;
  });
}

PdfTask<void> _compress(
  Pdf pdf,
  PdfInput input,
  DataSink output,
  PdfImagePolicy images,
) {
  KeepRecord.op('render');
  return PdfTask.group((hook) async {
    try {
      final lease = await input.opened;
      try {
        final source = lease.source;
        await hook.guard(
          _own(pdf.edit(PdfInput(source, password: input.password)), (
            editor,
            ownHook,
          ) async {
            await ownHook.guard(editor.reduceImages(images));
            const save = PdfSave.full(
              mergeDuplicates: true,
              pruneResources: true,
            );
            if (source.length <= _compressInMemoryLimit) {
              final result = MemorySink();
              await ownHook.guard(editor.save(result, save: save));
              final bytes = result.takeBytes();
              if (bytes.length < source.length) {
                for (var at = 0; at < bytes.length; at += _compressChunk) {
                  final end = min(at + _compressChunk, bytes.length);
                  await output.write(Uint8List.sublistView(bytes, at, end));
                }
              } else {
                await _copyUnchanged(source, output);
              }
            } else {
              final size = await ownHook.guard(
                editor._handle.measureSave(save: save),
              );
              if (size < source.length) {
                await ownHook.guard(editor.save(output, save: save));
              } else {
                await _copyUnchanged(source, output);
              }
            }
          }, output: output),
        );
      } finally {
        await lease.release();
      }
    } catch (error) {
      await hook.abortSinks([output], error);
      rethrow;
    }
  });
}

/// A source up to this size is compressed into memory and compared with the
/// source; a larger one is measured by the engine first.
const _compressInMemoryLimit = 4 * 1024 * 1024;

const _compressChunk = 64 * 1024;

Future<void> _copyUnchanged(DataSource source, DataSink output) async {
  for (var at = 0; at < source.length;) {
    final chunk = await source.readAt(
      at,
      min(_compressChunk, source.length - at),
    );
    if (chunk.isEmpty) {
      throw PdfIoFailed('source ended at $at of ${source.length} bytes');
    }
    await output.write(chunk);
    at += chunk.length;
  }
}

/// The page indices from [start] up to, not including, [end].
List<int> _pageRun(int start, int end) => [for (var p = start; p < end; p++) p];

/// How many pages from [start] make the longest part within [maxBytes]: the
/// engine measures the file of a page run, the run doubles while it fits,
/// and the last doubling is bisected. A single page is a part whatever its
/// size.
Future<int> _pagesThatFit(
  BridgeEditorHandle handle,
  int start,
  int total,
  int maxBytes,
) async {
  Future<bool> fits(int count) async =>
      await handle.measurePages(_pageRun(start, start + count)) <= maxBytes;

  final remaining = total - start;
  var probe = 1;
  var lastFit = 1;
  while (probe <= remaining) {
    if (await fits(probe)) {
      lastFit = probe;
      probe *= 2;
    } else {
      break;
    }
  }

  var lo = lastFit;
  var hi = probe.clamp(1, remaining);
  var best = lastFit;
  if (lo < hi) {
    while (lo <= hi) {
      final mid = (lo + hi) ~/ 2;
      if (await fits(mid)) {
        best = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
  }
  return best;
}
