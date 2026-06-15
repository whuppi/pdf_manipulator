/// File-backed `DataSource` / `DataSink` helpers for native platforms.
///
/// ```dart
/// import 'package:pdf_manipulator/pdf_manipulator.dart';
/// import 'package:pdf_manipulator/io.dart';
///
/// final doc = await pdf.open(FileSource(File('in.pdf')));
/// final out = await FileSink.create(File('out.pdf'));
/// ```
///
/// This library imports `dart:io`, so it is a separate entry point from the
/// main `package:pdf_manipulator/pdf_manipulator.dart` library. Web builds
/// simply don't import it and never pull in `dart:io`. On web, back sources
/// and sinks with a `Blob` or in-memory bytes instead.
library;

export 'src/io/file_sink.dart';
export 'src/io/file_source.dart';
