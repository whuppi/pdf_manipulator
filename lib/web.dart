/// Browser-backed `DataSource` helpers for web.
///
/// ```dart
/// import 'package:pdf_manipulator/pdf_manipulator.dart';
/// import 'package:pdf_manipulator/web.dart';
///
/// final input = PdfInput(BlobSource(file));
/// ```
///
/// This library imports `dart:js_interop`, so it is a separate entry point
/// from the main `package:pdf_manipulator/pdf_manipulator.dart` library, as
/// `io.dart` is for `dart:io`. Native builds simply don't import it.
library;

export 'src/io/blob_source.dart';
