import 'dart:io';

import 'package:pdf_manipulator/src/runtime/native/native_lane.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/shared_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Creates a [PdfBridge] backed by the native lane runtime.
PdfBridge createBridge({PdfConfig? config}) => SharedBridge(
  Router(
    host: NativeLaneHost(),
    maxLanes: _defaultMaxLanes(),
    mode: PdfIoMode.native,
  ),
);

/// One lane per two cores, at least two — leaves headroom for the
/// app's own threads; idle lanes sleep at zero CPU.
int _defaultMaxLanes() {
  final cores = Platform.numberOfProcessors;
  return cores ~/ 2 < 2 ? 2 : cores ~/ 2;
}
