import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/native/bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

PdfBridge createBridge({PdfConfig? config}) => NativeBridge();
