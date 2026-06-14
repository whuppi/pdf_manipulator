import 'package:web/web.dart' as web;

import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/runtime/web/web_lane.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/shared_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

/// Creates a [PdfBridge] backed by the web lane runtime.
PdfBridge createBridge({PdfConfig? config}) {
  final host = WebLaneHost(
    laneWorkerUrl: config?.webLaneWorkerUrl,
    forceMode: config?.webIoMode,
  );
  return SharedBridge(
    Router(
      host: host,
      maxLanes: config?.maxLanes ?? _defaultMaxLanes(),
      mode: host.mode,
    ),
  );
}

/// One lane per two cores, at least two — mirrors the native sizing.
int _defaultMaxLanes() {
  final cores = web.window.navigator.hardwareConcurrency;
  return cores ~/ 2 < 2 ? 2 : cores ~/ 2;
}
