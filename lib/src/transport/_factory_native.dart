import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:pdf_manipulator/src/transport/native/native_bridge.dart';

PdfBridge createBridge() => NativeBridge();
