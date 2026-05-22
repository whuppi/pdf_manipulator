// OPFS helpers — stream PdfSource to OPFS, cleanup registry.
//
// All OPFS operations go to a specific WebWorkerSession (same worker).
// One session = one operation = one OPFS file lifecycle.
// No cross-worker lock conflicts.
//
// INTERNAL — used by WebBridge only.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/bridge/web/worker_pool.dart';

/// Chunk size for streaming PdfSource data to OPFS.
const opfsChunkSize = 256 * 1024;

/// Tracks OPFS temp files for cleanup on error/dispose.
class OpfsRegistry {
  final _files = <String>{};
  static int _globalCounter = 0;

  String register() {
    final name = '_pdf_${DateTime.now().microsecondsSinceEpoch}_${_globalCounter++}.tmp';
    _files.add(name);
    return name;
  }

  void release(String name) => _files.remove(name);

  Set<String> get files => Set.unmodifiable(_files);

  void clear() => _files.clear();

  int get count => _files.length;
}

/// Stream a PdfSource's data to OPFS via a session's worker.
///
/// All write + finalize messages go to the same worker (the session).
/// Returns the OPFS filename.
Future<String> streamSourceToOpfs(
  PdfSource source,
  WebWorkerSession session,
  OpfsRegistry registry,
) async {
  final filename = registry.register();

  var offset = 0;
  while (offset < source.length) {
    final count = (source.length - offset).clamp(0, opfsChunkSize);
    final raw = await source.readAt(offset, count);
    // Always copy — postMessage transfer detaches the buffer.
    // The consumer's PdfSource must not have its buffer detached.
    final chunk = Uint8List.fromList(raw);
    await session.send('opfs.write', {
      'filename': filename,
      'chunk': chunk.buffer,
      'offset': offset,
    });
    offset += count;
  }

  await session.send('opfs.finalize', {'filename': filename});
  return filename;
}

/// Clean up a specific OPFS file via a session's worker.
Future<void> cleanupOpfsFile(
  String filename,
  WebWorkerSession session,
  OpfsRegistry registry,
) async {
  registry.release(filename);
  try {
    await session.send('opfs.cleanup', {'filename': filename});
  } catch (_) {}
}

/// Clean up ALL registered OPFS files via a session.
/// Used on dispose — best effort, ignore errors.
Future<void> cleanupAllOpfs(
  WebWorkerSession session,
  OpfsRegistry registry,
) async {
  for (final name in registry.files) {
    try {
      await session.send('opfs.cleanup', {'filename': name});
    } catch (_) {}
  }
  registry.clear();
}
