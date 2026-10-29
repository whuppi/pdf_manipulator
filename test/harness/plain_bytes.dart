import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

import 'test_source_sink.dart';

/// [pdf] rewritten with every object in it written plain: no object
/// streams, no stream compression, and nothing dropped as unreachable.
///
/// A save packs its objects into deflated object streams, so a claim about
/// which objects the file holds can only be read from bytes after this
/// rewrite. Garbage collection is off so an object [pdf] holds but nothing
/// reaches still shows.
Future<Uint8List> plainBytes(Pdf pdf, Uint8List bytes) async {
  final editor = await pdf.edit(src(bytes));
  final sink = TestSink();
  await editor.save(
    sink,
    options: const PdfSaveOptions.fullRewrite(
      compress: false,
      garbageCollect: false,
      objectStreams: false,
    ),
  );
  await editor.dispose();
  return sink.takeBytes();
}
