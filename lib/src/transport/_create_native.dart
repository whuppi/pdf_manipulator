import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/native/bridge.dart';

PdfBridge createBridge() => NativeBridge();
