import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/shared_bridge.dart';
import 'package:pdf_manipulator/src/transport/web/web_transport.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

/// Creates a [PdfBridge] backed by web WASM transport.
PdfBridge createBridge({PdfConfig? config}) =>
    SharedBridge(WebTransport(
      coordinatorUrl: config?.webCoordinatorUrl,
      workerUrl: config?.webWorkerUrl,
      ioMode: config?.webIoMode,
    ));
