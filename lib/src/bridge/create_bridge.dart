// Platform-neutral bridge factory: builds a SharedBridge over the shared
// Router, backed by the platform's LaneHost (the platform conditional import
// picks it). No dart:io, no package:web here — the platform split is the
// host's job.
//
// INTERNAL — used by the Pdf class to get a PdfBridge.

import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/shared_bridge.dart';
import 'package:pdf_manipulator/src/runtime/host.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

/// Creates a platform-appropriate [PdfBridge]. The lane host comes from the
/// platform conditional import; the Router + SharedBridge wrapping is shared.
PdfBridge createBridge({PdfConfig? config}) {
  final host = createLaneHost(config: config);
  return SharedBridge(
    Router(
      host: host,
      maxLanes: config?.maxLanes ?? host.defaultLaneCount,
      mode: host.mode,
    ),
  );
}
