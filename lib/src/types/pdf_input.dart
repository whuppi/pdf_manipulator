import 'dart:async';

import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';

/// A PDF to open: where its bytes come from and the password that unlocks
/// them. Every door that opens a PDF takes one, and nothing else names a
/// password.
///
/// ```dart
/// await pdf.merge([PdfInput(a), PdfInput(b, password: 'secret')], output);
/// ```
///
/// The default constructor borrows: the caller opened the source and closes it
/// when the job is over. [PdfInput.owning] hands the job the way to open it:
/// the job opens the source when it needs it and closes it when it is done
/// with it.
base class PdfInput {
  /// Opens the PDF in [source], with [password] when it is encrypted. The
  /// caller keeps the source and closes it.
  const PdfInput(DataSource source, {this.password})
    : _source = source,
      _open = null;

  /// Opens the PDF [open] returns, with [password] when it is encrypted. The
  /// job calls [open] when it needs the source and closes the source it
  /// returns when the job is done with it, on success, error and cancel.
  const PdfInput.owning(FutureOr<DataSource> Function() open, {this.password})
    : _source = null,
      _open = open;

  /// The password that opens the document, or null when it has none.
  final String? password;

  final DataSource? _source;
  final FutureOr<DataSource> Function()? _open;

  /// Opens the source if this input owns it, and returns it with the way to
  /// release it.
  @internal
  Future<PdfInputLease> get opened async {
    final open = _open;
    if (open == null) return PdfInputLease._(_source!, null);
    final source = await open();
    return PdfInputLease._(source, source.close);
  }

  @override
  bool operator ==(Object other) =>
      other is PdfInput &&
      other.runtimeType == runtimeType &&
      other._source == _source &&
      other._open == _open &&
      other.password == password;

  @override
  int get hashCode => Object.hash(_source, _open, password);
}

/// A source taken from a [PdfInput] for the length of one job.
@internal
final class PdfInputLease {
  PdfInputLease._(this.source, FutureOr<void> Function()? close)
    : _close = close;

  /// The source to read.
  final DataSource source;

  FutureOr<void> Function()? _close;

  /// Closes the source when the input owns it; a borrowed source is left to
  /// its caller. Idempotent.
  Future<void> release() async {
    final close = _close;
    _close = null;
    if (close != null) await close();
  }
}
