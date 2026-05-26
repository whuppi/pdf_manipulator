// Conditional import router — creates NativeBridge or WebBridge
// based on the platform at compile time.
//
// INTERNAL — used by the Pdf class to get a PdfBridge.

import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

import 'package:pdf_manipulator/src/transport/_create_native.dart'
    if (dart.library.js_interop) 'package:pdf_manipulator/src/transport/_create_web.dart'
    as impl;

PdfBridge createBridge({PdfConfig? config}) => impl.createBridge(config: config);
