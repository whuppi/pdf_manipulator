// Conditional import router — creates SharedBridge with the
// platform-specific transport (NativeTransport or WebTransport).
//
// INTERNAL — used by the Pdf class to get a PdfBridge.

import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

import 'package:pdf_manipulator/src/transport/_create_native.dart'
    if (dart.library.js_interop) 'package:pdf_manipulator/src/transport/_create_web.dart'
    as impl;

/// Creates a platform-appropriate [PdfBridge] via conditional import.
PdfBridge createBridge({PdfConfig? config}) => impl.createBridge(config: config);
