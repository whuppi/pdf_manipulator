// Conditional import router — creates NativeBridge or WebBridge
// based on the platform at compile time.
//
// INTERNAL — used by the Pdf class to get a PdfBridge.

import 'package:pdf_manipulator/src/bridge/bridge.dart';

import 'package:pdf_manipulator/src/bridge/_factory_native.dart'
    if (dart.library.js_interop) 'package:pdf_manipulator/src/bridge/_factory_web.dart'
    as impl;

PdfBridge createBridge() => impl.createBridge();
