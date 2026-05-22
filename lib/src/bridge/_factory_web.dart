import 'package:pdf_manipulator/src/bridge/bridge.dart';
import 'package:pdf_manipulator/src/bridge/web/web_bridge.dart';

PdfBridge createBridge() => WebBridge();
