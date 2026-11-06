// Platform-neutral bridge factory: builds a SharedBridge over the shared
// Router, backed by the shared ExecutorPool over the platform's ExecutorBooter
// (the platform conditional import picks it). No dart:io, no package:web here — the platform split is the
// host's job.
//
// INTERNAL — used by the Pdf class to get a PdfBridge.

import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/shared_bridge.dart';
import 'package:pdf_manipulator/src/ops/handle_backstop.dart';
import 'package:pdf_manipulator/src/runtime/host.dart';
import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

/// Creates a platform-appropriate [PdfBridge]. The lane host comes from the
/// platform conditional import and its executors from the pool every
/// instance shares; the Router + SharedBridge wrapping is shared.
PdfBridge createBridge({PdfConfig? config}) {
  final host = createBooter(config: config);
  final router = Router(
    pool: ExecutorPool.of(host),
    maxLanes: config?.concurrency ?? host.defaultConcurrency,
  );
  final bridge = SharedBridge(router);
  // The bridge is held by the Pdf and by every handle and op in flight, so
  // it is collected only when nothing can use the router any more. The
  // router holds the lanes, and with them the pool's executors and the
  // registered fonts, which nothing else would give back.
  HandleBackstop.attach(
    bridge,
    'Pdf',
    router.dispose,
    remedy: 'call dispose()',
  );
  return bridge;
}
