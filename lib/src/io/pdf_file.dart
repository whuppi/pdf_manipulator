import 'dart:io';

import 'package:pdf_manipulator/src/io/file_source.dart';
import 'package:pdf_manipulator/src/types/pdf_input.dart';

/// A PDF in a file on disk. The job opens the file when it needs it and
/// closes it when it is done, on success, error and cancel: a [FileSource]
/// you open yourself stays yours to close.
///
/// ```dart
/// await pdf.editOnce(PdfFile(File('in.pdf')), output).addWatermark('DRAFT');
/// ```
final class PdfFile extends PdfInput {
  /// Opens [file], with [password] when it is encrypted.
  PdfFile(File file, {super.password})
    : super.owning(() => FileSource.open(file));
}
