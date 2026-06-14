import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Platform-agnostic transport — moves binary request/response bytes
/// between SharedBridge and the Rust engine.
///
/// O(1)-memory I/O contract:
///   READ:  source bytes served on demand via DataSource (never buffered)
///   WRITE: output chunks received via DataSink (never buffered)
///   ARGS:  request/response bytes travel as Uint8List (~200 bytes per op)
///
/// Multi-source/multi-sink: any number of indexed sources and sinks
/// per operation. The platform routes readAt/chunk callbacks by index.
abstract interface class PdfTransport {
  /// Execute an operation. Returns binary response bytes.
  ///
  /// [sources] — indexed DataSources. sources[0] is typically the PDF,
  ///   sources[1+] are secondary inputs (merge source, image, etc.).
  ///   Each is served on demand via the platform's I/O channel.
  /// [sinks] — indexed DataSinks. sinks[0] is typically the primary
  ///   output. Each receives chunks via the platform's I/O channel.
  /// [keepSources] — indices of sources whose I/O resources should
  ///   persist after the call returns (for handle lifetime). Call
  ///   [releaseSource] when the handle is disposed.
  ///
  /// [cancel] — per-op cancellation. The transport binds it to the
  ///   submitted job; cancel-before-submit resolves the op as
  ///   cancelled without ever reaching a lane.
  ///
  /// Returns (responseBytes, resourceIds). resourceIds maps each kept
  /// source index to its resourceId for later release.
  Future<({Uint8List bytes, Map<int, int> resourceIds})> execute(
    Uint8List request, {
    List<DataSource> sources,
    List<DataSink> sinks,
    Set<int> keepSources,
    CancelHook? cancel,
  });

  /// Release source I/O resources held from a previous keepSources call.
  Future<void> releaseSource(int resourceId);

  /// Detected I/O mode after init. Null before first op.
  PdfIoMode? get ioMode;

  /// Eagerly initialize the transport. Returns the detected I/O mode.
  /// Idempotent — second call returns the cached result instantly.
  Future<PdfIoMode> ensureInitialized();

  /// Release all resources.
  Future<void> dispose();
}
