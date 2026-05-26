import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/web/bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

PdfBridge createBridge({PdfConfig? config}) => WebBridge(
  coordinatorUrl: config?.webCoordinatorUrl,
  workerUrl: config?.webWorkerUrl,
);
