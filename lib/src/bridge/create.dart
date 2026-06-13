// Conditional import router — creates SharedBridge over the shared
// Router, backed by the platform's LaneHost (native or web).
//
// INTERNAL — used by the Pdf class to get a PdfBridge.

import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

import 'package:pdf_manipulator/src/bridge/_create_native.dart'
    if (dart.library.js_interop) 'package:pdf_manipulator/src/bridge/_create_web.dart'
    as impl;

/// Creates a platform-appropriate [PdfBridge] via conditional import.
PdfBridge createBridge({PdfConfig? config}) =>
    impl.createBridge(config: config);
