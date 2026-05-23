import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:pdf_manipulator/src/transport/web/web_bridge.dart';

PdfBridge createBridge() => WebBridge();
