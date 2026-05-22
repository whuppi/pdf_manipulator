import 'package:pdf_manipulator/src/bridge/bridge.dart';
import 'package:pdf_manipulator/src/bridge/native/native_bridge.dart';

PdfBridge createBridge() => NativeBridge();
